import { afterEach, describe, expect, it, vi } from "vitest";

import { loadTheme, resolveIsDark, saveTheme } from "./theme";

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe("resolveIsDark", () => {
  it("follows the system only for the system choice", () => {
    expect(resolveIsDark("system", true)).toBe(true);
    expect(resolveIsDark("system", false)).toBe(false);
    expect(resolveIsDark("light", true)).toBe(false);
    expect(resolveIsDark("dark", false)).toBe(true);
  });
});

describe("remembering the choice", () => {
  it("defaults to system", () => {
    expect(loadTheme()).toBe("system");
  });

  it("round-trips a saved choice", () => {
    saveTheme("dark");
    expect(loadTheme()).toBe("dark");
  });

  it("ignores junk in storage", () => {
    window.localStorage.setItem("mfa-theme", "neon");
    expect(loadTheme()).toBe("system");
  });

  it("survives storage that throws", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(loadTheme()).toBe("system");
    expect(() => saveTheme("light")).not.toThrow();
  });
});
