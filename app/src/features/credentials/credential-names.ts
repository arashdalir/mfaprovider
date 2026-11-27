/**
 * The user names under which password managers keep the app's secrets (ADR-0021). All screens
 * share one web origin, so only the name tells two secrets apart: a manager that stores two of
 * them under one name overwrites one with the other. Renaming one orphans what users saved.
 */
export const ZIP_PASSWORD_NAME = "mfa-provider ZIP file password";

const DEVICE_CODE_KEY = "mfa-device-code";
const DEVICE_CODE = /^[0-9a-f]{4}$/;

/** Differs per installation, because every device has its own password. */
export function devicePasswordName(): string {
  const device = [deviceKind(window.navigator.userAgent), deviceCode()].filter(Boolean).join("-");
  return `mfa-provider device password (${device})`;
}

function deviceKind(userAgent: string): string {
  // Android's user agent also says Linux, so it has to be asked first.
  const kinds = ["Android", "Windows", "Mac", "Linux"];
  return kinds.find((kind) => userAgent.includes(kind))?.toLowerCase() ?? "device";
}

function deviceCode(): string | null {
  try {
    const stored = window.localStorage.getItem(DEVICE_CODE_KEY);
    if (stored !== null && DEVICE_CODE.test(stored)) {
      return stored;
    }
    const bytes = window.crypto.getRandomValues(new Uint8Array(2));
    const code = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
    window.localStorage.setItem(DEVICE_CODE_KEY, code);
    return code;
  } catch {
    return null;
  }
}
