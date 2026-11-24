import { IpcCallError, type IpcErrorCode } from "./client";

const MESSAGES: Record<IpcErrorCode, string> = {
  locked: "The vault is locked.",
  no_vault: "There is no vault yet.",
  already_exists: "A vault already exists.",
  wrong_password: "Wrong password.",
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
