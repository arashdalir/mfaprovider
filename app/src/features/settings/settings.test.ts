import { afterEach, describe, expect, it, vi } from "vitest";

import {
  DEFAULT_SETTINGS,
  idleLockSeconds,
  loadSettings,
  parseSettings,
  saveSettings,
} from "./settings";

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe("parseSettings", () => {
  it("keeps valid values, including 'never lock'", () => {
    expect(parseSettings({ idleLockMinutes: 15, clipboardClearSeconds: 60, hideCodes: false })).toEqual({
      idleLockMinutes: 15,
      clipboardClearSeconds: 60,
      hideCodes: false,
    });
    expect(parseSettings({}).hideCodes).toBe(true);
    expect(parseSettings({ hideCodes: "no" }).hideCodes).toBe(true);
    expect(parseSettings({ idleLockMinutes: null, clipboardClearSeconds: 10 }).idleLockMinutes).toBeNull();
  });

  it("replaces anything not on offer with the default", () => {
    expect(parseSettings({ idleLockMinutes: 7, clipboardClearSeconds: 0 })).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings({ idleLockMinutes: "5", clipboardClearSeconds: "30" })).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings("junk")).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings({})).toEqual(DEFAULT_SETTINGS);
  });
});

describe("storage", () => {
  it("defaults when nothing is stored", () => {
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("round-trips, including 'never lock'", () => {
    saveSettings({ idleLockMinutes: null, clipboardClearSeconds: 120, hideCodes: false });
    expect(loadSettings()).toEqual({ idleLockMinutes: null, clipboardClearSeconds: 120, hideCodes: false });
  });

  it("survives corrupt JSON and storage that throws", () => {
    window.localStorage.setItem("mfa-settings", "{not json");
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(() => saveSettings(DEFAULT_SETTINGS)).not.toThrow();
  });
});

describe("idleLockSeconds", () => {
  it("converts minutes and keeps 'never'", () => {
    expect(idleLockSeconds({ ...DEFAULT_SETTINGS, idleLockMinutes: 5 })).toBe(300);
    expect(idleLockSeconds({ ...DEFAULT_SETTINGS, idleLockMinutes: null })).toBeNull();
  });
});
