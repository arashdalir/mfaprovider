import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ZIP_PASSWORD_NAME, devicePasswordName } from "./credential-names";

const ANDROID = "Mozilla/5.0 (Linux; Android 16; Pixel 10a; wv) AppleWebKit/537.36";
const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

function runningOn(userAgent: string) {
  vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue(userAgent);
}

describe("devicePasswordName", () => {
  it("names the kind of device and a code of this installation", () => {
    runningOn(ANDROID);
    expect(devicePasswordName()).toMatch(/^mfa-provider device password \(android-[0-9a-f]{4}\)$/);
  });

  it("stays the same from one time to the next", () => {
    runningOn(WINDOWS);
    expect(devicePasswordName()).toBe(devicePasswordName());
    expect(devicePasswordName()).toContain("(windows-");
  });

  it("differs between two installations, so one cannot overwrite the other", () => {
    const random = vi.spyOn(window.crypto, "getRandomValues");
    const draw = (byte: number) =>
      random.mockImplementationOnce((array) => {
        (array as Uint8Array).fill(byte);
        return array;
      });
    draw(0x1a);
    const first = devicePasswordName();
    window.localStorage.clear();
    draw(0x2b);
    expect(devicePasswordName()).not.toBe(first);
  });

  it("leaves the code out when it cannot be kept, rather than changing every time", () => {
    runningOn(WINDOWS);
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(devicePasswordName()).toBe("mfa-provider device password (windows)");
  });

  it("is never the name of another secret", () => {
    expect(devicePasswordName()).not.toBe(ZIP_PASSWORD_NAME);
  });
});
