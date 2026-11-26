import { IpcCallError, type IpcErrorCode } from "./client";

const MESSAGES: Record<IpcErrorCode, string> = {
  locked: "The vault is locked.",
  no_vault: "There is no vault yet.",
  already_exists: "A vault already exists.",
  wrong_password: "Wrong password.",
  password_required: "This file needs a password.",
  password_due: "Please type your password this time.",
  biometric_unavailable: "Fingerprint or face unlock is not available on this device.",
  biometric_not_enabled: "Fingerprint or face unlock is not turned on.",
  biometric_cancelled: "Cancelled.",
  biometric_failed: "Fingerprint or face was not recognised.",
  biometric_invalidated:
    "The device's fingerprints or face changed, so this unlock was switched off. Use your password.",
  not_found: "That account no longer exists.",
  invalid_input: "Some of the input is not valid.",
  unsupported: "This account type is not supported by this version.",
  storage_failed: "The vault file could not be read or written.",
  corrupt_vault: "The vault file is damaged or from a newer version.",
};

/** User-facing text for any failure from the core; never includes secrets. */
export function describeError(error: unknown): string {
  if (error instanceof IpcCallError) {
    return error.code === "invalid_input" && error.message !== error.code
      ? error.message
      : MESSAGES[error.code];
  }
  return "Something went wrong.";
}

export function isLockedError(error: unknown): boolean {
  return error instanceof IpcCallError && error.code === "locked";
}

export function isPasswordRequired(error: unknown): boolean {
  return error instanceof IpcCallError && error.code === "password_required";
}
