# ChadVoice

**Fully local voice dictation with Aqua Voice-style ergonomics. Free, open source, offline.**

ChadVoice is a fork of [Handy](https://github.com/cjpais/Handy) by CJ Pais (MIT) that adds:

- **Hold or tap the same key**: hold Left Option for push-to-talk, or tap it to latch hands-free recording (tap again to stop)
- **Hands-free lock** on Fn+Space, **Esc** to cancel, **Cmd+Ctrl+V** to re-paste the last transcript
- **AI cleanup by default via local Ollama** (filler-word removal, self-corrections, punctuation) — no cloud, no API key
- **Find→replace rules** applied to every transcript, plus the personal dictionary
- **No phone-home**: the auto-updater is removed; the only network use is the one-time model download and your localhost LLM

Press a shortcut, speak, and your words appear in any text field — entirely on your own computer.

## Installing

Grab the latest DMG (macOS, Apple Silicon or Intel) or installer (Windows x64/ARM64) from [Releases](https://github.com/vietchad/chadvoice/releases).

Builds are unsigned community builds:

- **macOS**: right-click the app → Open (or run `xattr -cr /Applications/ChadVoice.app`) the first time
- **Windows**: click "More info" → "Run anyway" if SmartScreen appears

For the optional AI cleanup, install [Ollama](https://ollama.com) and run `ollama pull qwen2.5:7b` (configurable in Settings → Post-Processing; toggle it off for raw transcription).
