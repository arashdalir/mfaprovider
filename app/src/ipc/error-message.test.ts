import { describe, expect, it } from "vitest";

import { IpcCallError } from "./client";
import { describeError, isLockedError } from "./error-message";

describe("describeError", () => {
  it("has a fixed message for every code", () => {
    expect(describeError(new IpcCallError({ code: "wrong_password", message: null }))).toBe(
      "Wrong password.",
    );
    expect(describeError(new IpcCallError({ code: "storage_failed", message: null }))).toMatch(
      /could not be read or written/,
    );
  });

  it("shows the core's own text for invalid input", () => {
    const error = new IpcCallError({ code: "invalid_input", message: "the secret is empty" });
    expect(describeError(error)).toBe("the secret is empty");
  });

  it("falls back to a generic message for unknown failures", () => {
    expect(describeError(new Error("boom with details"))).toBe("Something went wrong.");
  });

  it("recognises the locked error", () => {
    expect(isLockedError(new IpcCallError({ code: "locked", message: null }))).toBe(true);
    expect(isLockedError(new Error("locked"))).toBe(false);
  });
});
