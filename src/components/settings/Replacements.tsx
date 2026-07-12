import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";
import type { TextReplacement } from "@/bindings";

interface ReplacementsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

interface ReplacementRowProps {
  replacement: TextReplacement;
  disabled: boolean;
  onCommit: (replacement: TextReplacement) => void;
  onRemove: () => void;
}

const ReplacementRow: React.FC<ReplacementRowProps> = ({
  replacement,
  disabled,
  onCommit,
  onRemove,
}) => {
  const { t } = useTranslation();
  const [from, setFrom] = useState(replacement.from);
  const [to, setTo] = useState(replacement.to);

  useEffect(() => {
    setFrom(replacement.from);
    setTo(replacement.to);
  }, [replacement.from, replacement.to]);

  const handleCommit = () => {
    const trimmedFrom = from.trim();
    if (!trimmedFrom) {
      setFrom(replacement.from);
      return;
    }
    if (trimmedFrom !== replacement.from || to !== replacement.to) {
      onCommit({ from: trimmedFrom, to });
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      e.currentTarget.blur();
    }
  };

  return (
    <div className="flex items-center gap-2">
      <Input
        type="text"
        className="max-w-40"
        value={from}
        onChange={(e) => setFrom(e.target.value)}
        onBlur={handleCommit}
        onKeyDown={handleKeyDown}
        placeholder={t("settings.advanced.replacements.fromPlaceholder")}
        variant="compact"
        disabled={disabled}
      />
      <span className="text-sm text-mid-gray">→</span>
      <Input
        type="text"
        className="max-w-40"
        value={to}
        onChange={(e) => setTo(e.target.value)}
        onBlur={handleCommit}
        onKeyDown={handleKeyDown}
        placeholder={t("settings.advanced.replacements.toPlaceholder")}
        variant="compact"
        disabled={disabled}
      />
      <Button
        onClick={onRemove}
        disabled={disabled}
        variant="secondary"
        size="sm"
        className="inline-flex items-center cursor-pointer"
        aria-label={t("settings.advanced.replacements.remove", {
          from: replacement.from,
        })}
      >
        <svg
          className="w-3 h-3"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M6 18L18 6M6 6l12 12"
          />
        </svg>
      </Button>
    </div>
  );
};

export const Replacements: React.FC<ReplacementsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [newFrom, setNewFrom] = useState("");
    const [newTo, setNewTo] = useState("");
    const replacements = getSetting("replacements") || [];

    const handleAddReplacement = () => {
      const trimmedFrom = newFrom.trim();
      if (!trimmedFrom) {
        return;
      }
      if (replacements.some((r) => r.from === trimmedFrom)) {
        toast.error(
          t("settings.advanced.replacements.duplicate", {
            from: trimmedFrom,
          }),
        );
        return;
      }
      updateSetting("replacements", [
        ...replacements,
        { from: trimmedFrom, to: newTo },
      ]);
      setNewFrom("");
      setNewTo("");
    };

    const handleUpdateReplacement = (
      index: number,
      replacement: TextReplacement,
    ) => {
      const updated = [...replacements];
      updated[index] = replacement;
      updateSetting("replacements", updated);
    };

    const handleRemoveReplacement = (index: number) => {
      updateSetting(
        "replacements",
        replacements.filter((_, i) => i !== index),
      );
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAddReplacement();
      }
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.replacements.title")}
          description={t("settings.advanced.replacements.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <Input
              type="text"
              className="max-w-40"
              value={newFrom}
              onChange={(e) => setNewFrom(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={t("settings.advanced.replacements.fromPlaceholder")}
              variant="compact"
              disabled={isUpdating("replacements")}
            />
            <span className="text-sm text-mid-gray">→</span>
            <Input
              type="text"
              className="max-w-40"
              value={newTo}
              onChange={(e) => setNewTo(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={t("settings.advanced.replacements.toPlaceholder")}
              variant="compact"
              disabled={isUpdating("replacements")}
            />
            <Button
              onClick={handleAddReplacement}
              disabled={!newFrom.trim() || isUpdating("replacements")}
              variant="primary"
              size="md"
            >
              {t("settings.advanced.replacements.add")}
            </Button>
          </div>
        </SettingContainer>
        {replacements.length > 0 && (
          <div
            className={`px-4 p-2 ${grouped ? "" : "rounded-lg border border-mid-gray/20"} flex flex-col gap-2`}
          >
            {replacements.map((replacement, index) => (
              <ReplacementRow
                key={index}
                replacement={replacement}
                disabled={isUpdating("replacements")}
                onCommit={(updated) => handleUpdateReplacement(index, updated)}
                onRemove={() => handleRemoveReplacement(index)}
              />
            ))}
          </div>
        )}
      </>
    );
  },
);
