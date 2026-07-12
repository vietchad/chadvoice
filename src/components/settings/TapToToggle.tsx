import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface TapToToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const TapToToggle: React.FC<TapToToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const tapToToggleEnabled = getSetting("tap_to_toggle") ?? true;

    return (
      <ToggleSwitch
        checked={tapToToggleEnabled}
        onChange={(enabled) => updateSetting("tap_to_toggle", enabled)}
        isUpdating={isUpdating("tap_to_toggle")}
        label={t("settings.general.tapToToggle.label")}
        description={t("settings.general.tapToToggle.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
