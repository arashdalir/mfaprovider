export interface Settings {
  /** Minutes of inactivity before the vault locks; null turns the idle lock off. */
  idleLockMinutes: number | null;
  clipboardClearSeconds: number;
  /** Codes stay masked until the user presses Show. */
  hideCodes: boolean;
}

export const IDLE_LOCK_OPTIONS: (number | null)[] = [1, 5, 15, 30, null];
export const CLIPBOARD_CLEAR_OPTIONS = [10, 30, 60, 120];

export const DEFAULT_SETTINGS: Settings = { idleLockMinutes: 5, clipboardClearSeconds: 30,
  hideCodes: true,
};

const STORAGE_KEY = "mfa-settings";

/** Each field must be one of the offered values, otherwise its default applies. */
export function parseSettings(raw: unknown): Settings {
  const candidate = typeof raw === "object" && raw !== null ? (raw as Record<string, unknown>) : {};
  const idle = candidate.idleLockMinutes as number | null;
  const clipboard = candidate.clipboardClearSeconds as number;
  return {
    idleLockMinutes: IDLE_LOCK_OPTIONS.includes(idle) ? idle : DEFAULT_SETTINGS.idleLockMinutes,
    clipboardClearSeconds: CLIPBOARD_CLEAR_OPTIONS.includes(clipboard)
      ? clipboard
      : DEFAULT_SETTINGS.clipboardClearSeconds,
    hideCodes: typeof candidate.hideCodes === "boolean" ? candidate.hideCodes : DEFAULT_SETTINGS.hideCodes,
  };
}

export function loadSettings(): Settings {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return parseSettings(stored === null ? null : JSON.parse(stored));
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(settings: Settings): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // The choice then lasts for this session only.
  }
}

export function idleLockSeconds(settings: Settings): number | null {
  return settings.idleLockMinutes === null ? null : settings.idleLockMinutes * 60;
}
