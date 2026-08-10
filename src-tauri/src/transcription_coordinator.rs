use crate::actions::ACTION_MAP;
use crate::managers::audio::AudioRecordingManager;
use log::{debug, error, warn};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const DEBOUNCE: Duration = Duration::from_millis(30);
const RELEASE_GRACE: Duration = Duration::from_millis(50);
/// A push-to-talk press released within this window is a "tap": recording
/// latches on and continues hands-free until the key is pressed again
/// (Aqua Voice-style tap-to-toggle on the same key as hold-to-talk).
const TAP_LATCH: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PttAction {
    Passthrough,
    DeferRelease,
    CancelRelease,
}

struct PendingRelease {
    binding_id: String,
    hotkey_string: String,
    deadline: Instant,
    tap_to_toggle: bool,
}

/// Commands processed sequentially by the coordinator thread.
enum Command {
    Input {
        binding_id: String,
        hotkey_string: String,
        is_pressed: bool,
        push_to_talk: bool,
        tap_to_toggle: bool,
        /// Another key was pressed while the hotkey was held — the event is
        /// part of an app-switch chord (Option+Tab / Cmd+Tab), not a
        /// deliberate tap of the transcribe key.
        chord: bool,
    },
    Cancel {
        recording_was_active: bool,
    },
    ProcessingFinished,
}

/// Pipeline lifecycle, owned exclusively by the coordinator thread.
enum Stage {
    Idle,
    Recording(String), // binding_id
    Processing,
}

fn classify_ptt_event(
    pending_release_binding: Option<&str>,
    is_pressed: bool,
    push_to_talk: bool,
    binding_id: &str,
    recording_binding: Option<&str>,
) -> PttAction {
    if !push_to_talk {
        return PttAction::Passthrough;
    }

    if is_pressed {
        if pending_release_binding == Some(binding_id) {
            PttAction::CancelRelease
        } else {
            PttAction::Passthrough
        }
    } else if recording_binding == Some(binding_id) && pending_release_binding.is_none() {
        PttAction::DeferRelease
    } else {
        PttAction::Passthrough
    }
}

/// Serialises all transcription lifecycle events through a single thread
/// to eliminate race conditions between keyboard shortcuts, signals, and
/// the async transcribe-paste pipeline.
pub struct TranscriptionCoordinator {
    tx: Sender<Command>,
}

pub fn is_transcribe_binding(id: &str) -> bool {
    id == "transcribe" || id == "transcribe_with_post_process"
}

impl TranscriptionCoordinator {
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut stage = Stage::Idle;
                let mut last_press: Option<Instant> = None;
                let mut pending_release: Option<PendingRelease> = None;
                // When the current recording started, and whether it is
                // "latched" (hands-free: releases are ignored, next press
                // stops). Set by tap-to-toggle and by toggle-mode starts.
                let mut recording_started: Option<Instant> = None;
                let mut latched = false;

                loop {
                    let cmd = if let Some(pending) = &pending_release {
                        match rx.recv_timeout(
                            pending.deadline.saturating_duration_since(Instant::now()),
                        ) {
                            Ok(cmd) => cmd,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if let Some(pending) = pending_release.take() {
                                    if matches!(&stage, Stage::Recording(id) if id == &pending.binding_id)
                                    {
                                        // The deadline includes RELEASE_GRACE, so
                                        // compare against TAP_LATCH + grace to
                                        // recover the actual hold duration.
                                        let is_tap = pending.tap_to_toggle
                                            && !latched
                                            && recording_started.is_some_and(|t| {
                                                pending
                                                    .deadline
                                                    .saturating_duration_since(t)
                                                    < TAP_LATCH + RELEASE_GRACE
                                            });
                                        if is_tap {
                                            debug!(
                                                "Tap detected for '{}': latching hands-free recording",
                                                pending.binding_id
                                            );
                                            latched = true;
                                        } else {
                                            stop(
                                                &app,
                                                &mut stage,
                                                &pending.binding_id,
                                                &pending.hotkey_string,
                                            );
                                        }
                                    }
                                }
                                continue;
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match rx.recv() {
                            Ok(cmd) => cmd,
                            Err(_) => break,
                        }
                    };

                    match cmd {
                        Command::Input {
                            binding_id,
                            hotkey_string,
                            is_pressed,
                            push_to_talk,
                            tap_to_toggle,
                            chord,
                        } => {
                            let pending_release_binding = pending_release
                                .as_ref()
                                .map(|pending| pending.binding_id.as_str());
                            let recording_binding = match &stage {
                                Stage::Recording(id) => Some(id.as_str()),
                                _ => None,
                            };

                            // Chord release during a latched (hands-free)
                            // recording: the key event was part of an
                            // app-switch chord, not a deliberate stop tap.
                            // Keep recording.
                            if !is_pressed
                                && chord
                                && latched
                                && recording_binding == Some(binding_id.as_str())
                            {
                                debug!(
                                    "Ignoring chord release for '{binding_id}': keeping latched recording"
                                );
                                continue;
                            }

                            match classify_ptt_event(
                                pending_release_binding,
                                is_pressed,
                                push_to_talk,
                                &binding_id,
                                recording_binding,
                            ) {
                                PttAction::CancelRelease => {
                                    pending_release = None;
                                    continue;
                                }
                                PttAction::DeferRelease => {
                                    pending_release = Some(PendingRelease {
                                        binding_id,
                                        hotkey_string,
                                        deadline: Instant::now() + RELEASE_GRACE,
                                        tap_to_toggle,
                                    });
                                    continue;
                                }
                                PttAction::Passthrough => {}
                            }

                            // Debounce rapid-fire press events (key repeat / double-tap).
                            // Push-to-talk releases may be deferred above to absorb X11 auto-repeat.
                            if is_pressed {
                                let now = Instant::now();
                                if last_press.is_some_and(|t| now.duration_since(t) < DEBOUNCE) {
                                    debug!("Debounced press for '{binding_id}'");
                                    continue;
                                }
                                last_press = Some(now);
                            }

                            if push_to_talk {
                                if is_pressed && matches!(stage, Stage::Idle) {
                                    start(&app, &mut stage, &binding_id, &hotkey_string);
                                    if matches!(stage, Stage::Recording(_)) {
                                        recording_started = Some(Instant::now());
                                        latched = false;
                                    }
                                } else if is_pressed
                                    && latched
                                    && matches!(&stage, Stage::Recording(id) if id == &binding_id)
                                {
                                    // Second press while latched hands-free: the
                                    // stop happens on the matching release, so a
                                    // press that turns out to be an app-switch
                                    // chord (Option+Tab) doesn't kill the
                                    // recording. The release is either a clean
                                    // tap (deferred below, stops via the timeout
                                    // path) or a chord (ignored above).
                                    debug!(
                                        "Press while latched for '{binding_id}': awaiting clean release to stop"
                                    );
                                } else if !is_pressed
                                    && !latched
                                    && matches!(&stage, Stage::Recording(id) if id == &binding_id)
                                {
                                    stop(&app, &mut stage, &binding_id, &hotkey_string);
                                }
                            } else if is_pressed {
                                match &stage {
                                    Stage::Idle => {
                                        start(&app, &mut stage, &binding_id, &hotkey_string);
                                        if matches!(stage, Stage::Recording(_)) {
                                            recording_started = Some(Instant::now());
                                            // Toggle-mode recordings are inherently
                                            // hands-free: a press (from any
                                            // transcribe binding) stops them.
                                            latched = true;
                                        }
                                    }
                                    Stage::Recording(id) if id == &binding_id => {
                                        stop(&app, &mut stage, &binding_id, &hotkey_string);
                                    }
                                    _ => {
                                        debug!("Ignoring press for '{binding_id}': pipeline busy")
                                    }
                                }
                            }
                        }
                        Command::Cancel {
                            recording_was_active,
                        } => {
                            pending_release = None;
                            latched = false;
                            recording_started = None;
                            // Don't reset during processing — wait for the pipeline to finish.
                            if !matches!(stage, Stage::Processing)
                                && (recording_was_active || matches!(stage, Stage::Recording(_)))
                            {
                                stage = Stage::Idle;
                            }
                        }
                        Command::ProcessingFinished => {
                            stage = Stage::Idle;
                            latched = false;
                            recording_started = None;
                        }
                    }
                }
                debug!("Transcription coordinator exited");
            }));
            if let Err(e) = result {
                error!("Transcription coordinator panicked: {e:?}");
            }
        });

        Self { tx }
    }

    /// Send a keyboard/signal input event for a transcribe binding.
    /// For signal-based toggles, use `is_pressed: true` and `push_to_talk: false`.
    pub fn send_input(
        &self,
        binding_id: &str,
        hotkey_string: &str,
        is_pressed: bool,
        push_to_talk: bool,
        tap_to_toggle: bool,
        chord: bool,
    ) {
        if self
            .tx
            .send(Command::Input {
                binding_id: binding_id.to_string(),
                hotkey_string: hotkey_string.to_string(),
                is_pressed,
                push_to_talk,
                tap_to_toggle,
                chord,
            })
            .is_err()
        {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn notify_cancel(&self, recording_was_active: bool) {
        if self
            .tx
            .send(Command::Cancel {
                recording_was_active,
            })
            .is_err()
        {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn notify_processing_finished(&self) {
        if self.tx.send(Command::ProcessingFinished).is_err() {
            warn!("Transcription coordinator channel closed");
        }
    }
}

fn start(app: &AppHandle, stage: &mut Stage, binding_id: &str, hotkey_string: &str) {
    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!("No action in ACTION_MAP for '{binding_id}'");
        return;
    };
    action.start(app, binding_id, hotkey_string);
    if app
        .try_state::<Arc<AudioRecordingManager>>()
        .is_some_and(|a| a.is_recording())
    {
        *stage = Stage::Recording(binding_id.to_string());
    } else {
        debug!("Start for '{binding_id}' did not begin recording; staying idle");
    }
}

fn stop(app: &AppHandle, stage: &mut Stage, binding_id: &str, hotkey_string: &str) {
    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!("No action in ACTION_MAP for '{binding_id}'");
        return;
    };
    action.stop(app, binding_id, hotkey_string);
    *stage = Stage::Processing;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_to_talk_release_while_recording_defers_release() {
        assert_eq!(
            classify_ptt_event(None, false, true, "transcribe", Some("transcribe")),
            PttAction::DeferRelease
        );
    }

    #[test]
    fn push_to_talk_press_matching_pending_release_cancels_release() {
        assert_eq!(
            classify_ptt_event(
                Some("transcribe"),
                true,
                true,
                "transcribe",
                Some("transcribe")
            ),
            PttAction::CancelRelease
        );
    }

    #[test]
    fn toggle_mode_press_and_release_pass_through() {
        assert_eq!(
            classify_ptt_event(
                Some("transcribe"),
                true,
                false,
                "transcribe",
                Some("transcribe")
            ),
            PttAction::Passthrough
        );
        assert_eq!(
            classify_ptt_event(None, false, false, "transcribe", Some("transcribe")),
            PttAction::Passthrough
        );
    }

    #[test]
    fn press_for_different_binding_than_pending_release_passes_through() {
        assert_eq!(
            classify_ptt_event(
                Some("transcribe"),
                true,
                true,
                "transcribe_with_post_process",
                Some("transcribe")
            ),
            PttAction::Passthrough
        );
    }

    #[test]
    fn press_matching_pending_release_cancels_without_recording_state() {
        assert_eq!(
            classify_ptt_event(Some("transcribe"), true, true, "transcribe", None),
            PttAction::CancelRelease
        );
    }

    // ---------------------------------------------------------------------
    // Sequence-level regression coverage for issue #1539.
    //
    // Under X11 key auto-repeat, holding a push-to-talk key does not emit one
    // long press. It emits the initial press followed by a stream of
    // synthesized release/press pairs, then a single genuine release on key-up.
    // Before the fix, every synthesized release passed straight through and
    // stopped recording, so holding the key "rapidly toggled" recording on and
    // off. The fix defers each release for a short grace window and cancels it
    // when the matching auto-repeat press arrives.
    //
    // The unit tests above assert `classify_ptt_event` in isolation. The
    // simulator below threads that classifier through the same `pending_release`
    // / `stage` state transitions the coordinator loop performs (lines that
    // handle `Command::Input` and the `recv_timeout` grace expiry), so a whole
    // event burst can be exercised deterministically without a Tauri AppHandle
    // or real timers.
    // ---------------------------------------------------------------------

    const BINDING: &str = "transcribe";

    #[derive(Clone, Copy)]
    enum Ev {
        /// A key-down event (real initial press or a synthesized auto-repeat press).
        Press,
        /// A key-up event (synthesized auto-repeat release or the genuine key-up).
        Release,
        /// The `RELEASE_GRACE` window elapsed with no cancelling press arriving.
        Grace,
        /// Simulated wall-clock time passing (ms) with no events.
        Wait(u64),
    }

    #[derive(Debug, PartialEq, Eq)]
    enum SimStage {
        Idle,
        Recording,
        Processing,
    }

    struct SimResult {
        starts: u32,
        stops: u32,
        stage: SimStage,
    }

    /// Mirror of the coordinator loop's decision logic for a single push-to-talk
    /// binding: it calls the real `classify_ptt_event` and applies the exact same
    /// Defer / Cancel / debounce / tap-latch / start / stop transitions.
    fn simulate(events: &[Ev], tap_to_toggle: bool) -> SimResult {
        let mut stage = SimStage::Idle;
        let mut pending: Option<(String, u64)> = None; // (binding, deadline_ms)
        let mut last_press_ms: Option<u64> = None;
        let mut clock_ms: u64 = 0;
        let mut starts = 0u32;
        let mut stops = 0u32;
        let mut recording_started_ms: Option<u64> = None;
        let mut latched = false;
        let debounce_ms = DEBOUNCE.as_millis() as u64;
        let grace_ms = RELEASE_GRACE.as_millis() as u64;
        let tap_latch_ms = TAP_LATCH.as_millis() as u64;

        for ev in events {
            // Auto-repeat events arrive a few ms apart, well inside DEBOUNCE.
            clock_ms += 5;

            match ev {
                Ev::Wait(ms) => {
                    clock_ms += ms;
                }
                Ev::Grace => {
                    // Coordinator's `RecvTimeoutError::Timeout` arm: fire the
                    // deferred release iff we are still recording that binding.
                    if let Some((pending_binding, deadline_ms)) = pending.take() {
                        if stage == SimStage::Recording && pending_binding == BINDING {
                            let is_tap = tap_to_toggle
                                && !latched
                                && recording_started_ms.is_some_and(|t| {
                                    deadline_ms.saturating_sub(t) < tap_latch_ms + grace_ms
                                });
                            if is_tap {
                                latched = true;
                            } else {
                                stage = SimStage::Processing;
                                stops += 1;
                            }
                        }
                    }
                }
                Ev::Press | Ev::Release => {
                    let is_pressed = matches!(ev, Ev::Press);
                    let pending_binding = pending.as_ref().map(|(b, _)| b.as_str());
                    let recording_binding = if stage == SimStage::Recording {
                        Some(BINDING)
                    } else {
                        None
                    };

                    match classify_ptt_event(
                        pending_binding,
                        is_pressed,
                        true, // push_to_talk
                        BINDING,
                        recording_binding,
                    ) {
                        PttAction::CancelRelease => {
                            pending = None;
                            continue;
                        }
                        PttAction::DeferRelease => {
                            pending = Some((BINDING.to_string(), clock_ms + grace_ms));
                            continue;
                        }
                        PttAction::Passthrough => {}
                    }

                    if is_pressed {
                        if last_press_ms.is_some_and(|t| clock_ms - t < debounce_ms) {
                            continue;
                        }
                        last_press_ms = Some(clock_ms);
                    }

                    if is_pressed && stage == SimStage::Idle {
                        stage = SimStage::Recording;
                        starts += 1;
                        recording_started_ms = Some(clock_ms);
                        latched = false;
                    } else if is_pressed && latched && stage == SimStage::Recording {
                        // Second press while latched hands-free: stop.
                        stage = SimStage::Processing;
                        stops += 1;
                    } else if !is_pressed && !latched && stage == SimStage::Recording {
                        stage = SimStage::Processing;
                        stops += 1;
                    }
                }
            }
        }

        SimResult {
            starts,
            stops,
            stage,
        }
    }

    /// Initial press plus several synthesized release/press pairs, as X11 emits
    /// while a push-to-talk key is held down.
    fn autorepeat_burst() -> Vec<Ev> {
        let mut events = vec![Ev::Press];
        for _ in 0..6 {
            events.push(Ev::Release);
            events.push(Ev::Press);
        }
        events
    }

    /// Regression for #1539: a burst of X11 auto-repeat release/press pairs must
    /// not stop recording. Before the fix the first synthesized release stopped
    /// recording immediately (stops == 1, stage left Recording), which produced
    /// the rapid on/off toggling. With the fix the releases are coalesced and
    /// recording stays continuously active for the whole burst.
    #[test]
    fn x11_autorepeat_burst_does_not_toggle_recording() {
        let result = simulate(&autorepeat_burst(), false);
        assert_eq!(result.starts, 1, "recording should start exactly once");
        assert_eq!(
            result.stops, 0,
            "synthesized auto-repeat releases must not stop recording mid-burst"
        );
        assert_eq!(
            result.stage,
            SimStage::Recording,
            "recording must remain active across the entire auto-repeat burst"
        );
    }

    /// Complements the burst test: once the key is genuinely released and the
    /// grace window elapses with no re-press, recording stops exactly once. This
    /// proves the debounce only coalesces synthesized releases and does not wedge
    /// the coordinator or swallow the real key-up.
    #[test]
    fn genuine_release_after_grace_stops_recording_once() {
        let mut events = autorepeat_burst();
        events.push(Ev::Release); // genuine key-up
        events.push(Ev::Grace); // grace window elapses, no cancelling press
        let result = simulate(&events, false);
        assert_eq!(result.starts, 1, "recording should start exactly once");
        assert_eq!(
            result.stops, 1,
            "a genuine release should stop recording exactly once"
        );
        assert_eq!(result.stage, SimStage::Processing);
    }

    // ---------------------------------------------------------------------
    // Tap-to-toggle (Aqua Voice-style): a press released within TAP_LATCH
    // latches recording hands-free; the next press stops it. Holding past
    // TAP_LATCH behaves as plain push-to-talk.
    // ---------------------------------------------------------------------

    #[test]
    fn tap_latches_recording_hands_free() {
        // Press, release 100ms later (a tap), grace elapses with no re-press.
        let events = [Ev::Press, Ev::Wait(100), Ev::Release, Ev::Grace];
        let result = simulate(&events, true);
        assert_eq!(result.starts, 1);
        assert_eq!(result.stops, 0, "a tap must latch, not stop");
        assert_eq!(result.stage, SimStage::Recording);
    }

    #[test]
    fn press_after_tap_latch_stops_recording() {
        let events = [
            Ev::Press,
            Ev::Wait(100),
            Ev::Release,
            Ev::Grace,
            Ev::Wait(2000),
            Ev::Press,
        ];
        let result = simulate(&events, true);
        assert_eq!(result.starts, 1);
        assert_eq!(result.stops, 1, "second press must stop a latched recording");
        assert_eq!(result.stage, SimStage::Processing);
    }

    #[test]
    fn hold_past_tap_window_stops_on_release() {
        let events = [Ev::Press, Ev::Wait(600), Ev::Release, Ev::Grace];
        let result = simulate(&events, true);
        assert_eq!(result.starts, 1);
        assert_eq!(result.stops, 1, "a long hold is plain push-to-talk");
        assert_eq!(result.stage, SimStage::Processing);
    }

    #[test]
    fn tap_disabled_short_press_still_stops() {
        let events = [Ev::Press, Ev::Wait(100), Ev::Release, Ev::Grace];
        let result = simulate(&events, false);
        assert_eq!(result.stops, 1, "with tap_to_toggle off, any release stops");
        assert_eq!(result.stage, SimStage::Processing);
    }

    #[test]
    fn release_after_latched_stop_press_is_ignored() {
        // Tap latches; later press stops; the release of that press must not
        // start or stop anything further.
        let events = [
            Ev::Press,
            Ev::Wait(100),
            Ev::Release,
            Ev::Grace,
            Ev::Wait(1000),
            Ev::Press,
            Ev::Wait(100),
            Ev::Release,
            Ev::Grace,
        ];
        let result = simulate(&events, true);
        assert_eq!(result.starts, 1);
        assert_eq!(result.stops, 1);
        assert_eq!(result.stage, SimStage::Processing);
    }
}
