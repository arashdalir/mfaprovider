import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { ThemeSwitch } from "@/features/theme/ThemeSwitch";
import type { Theme } from "@/features/theme/theme";
import { useApi } from "@/ipc/api-context";
import { describeError } from "@/ipc/error-message";

import {
  CLIPBOARD_CLEAR_OPTIONS,
  IDLE_LOCK_OPTIONS,
  idleLockSeconds,
  loadSettings,
  saveSettings,
  type Settings,
} from "./settings";

const idleLabel = (minutes: number | null) =>
  minutes === null ? "Never" : minutes === 1 ? "1 minute" : `${minutes} minutes`;

export function SettingsScreen({
  theme,
  onThemeChange,
  onClose,
}: {
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  onClose: () => void;
}) {
  const api = useApi();
  const [settings, setSettings] = useState<Settings>(loadSettings);
  const [problem, setProblem] = useState<string | null>(null);

  async function update(next: Settings) {
    setSettings(next);
    saveSettings(next);
    try {
      await api.setIdleLock(idleLockSeconds(next));
      setProblem(null);
    } catch (error) {
      setProblem(describeError(error));
    }
  }

  return (
    <section aria-label="Settings" className="flex flex-col gap-4">
      <h2 className="text-lg font-semibold">Settings</h2>
      <div className="flex flex-col gap-2">
        <Label htmlFor="idle-lock">Lock automatically after</Label>
        <select
          id="idle-lock"
          className="rounded-md border bg-background px-2 py-2"
          value={settings.idleLockMinutes ?? "never"}
          onChange={(event) =>
            void update({
              ...settings,
              idleLockMinutes: event.target.value === "never" ? null : Number(event.target.value),
            })
          }
        >
          {IDLE_LOCK_OPTIONS.map((minutes) => (
            <option key={minutes ?? "never"} value={minutes ?? "never"}>
              {idleLabel(minutes)}
            </option>
          ))}
        </select>
        <p className="text-sm text-muted-foreground">
          The vault also locks when the app goes to the background.
        </p>
      </div>
      <div className="flex flex-col gap-2">
        <Label htmlFor="clipboard-clear">Clear copied codes after</Label>
        <select
          id="clipboard-clear"
          className="rounded-md border bg-background px-2 py-2"
          value={settings.clipboardClearSeconds}
          onChange={(event) =>
            void update({ ...settings, clipboardClearSeconds: Number(event.target.value) })
          }
        >
          {CLIPBOARD_CLEAR_OPTIONS.map((seconds) => (
            <option key={seconds} value={seconds}>
              {seconds} seconds
            </option>
          ))}
        </select>
      </div>
      <div className="flex items-center gap-2">
        <input
          id="hide-codes"
          type="checkbox"
          checked={settings.hideCodes}
          onChange={(event) => void update({ ...settings, hideCodes: event.target.checked })}
        />
        <Label htmlFor="hide-codes">Hide codes until I press Show</Label>
      </div>
      <ThemeSwitch theme={theme} onChange={onThemeChange} />
      {problem && (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      )}
      <Button className="self-start" onClick={onClose}>
        Done
      </Button>
    </section>
  );
}
