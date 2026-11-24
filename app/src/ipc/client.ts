// The only module that talks to the Rust core (ADR-0012). Everything the UI may ask for is a
// typed function here; `no-restricted-imports` stops other files importing Tauri's `invoke`.
import { invoke } from "@tauri-apps/api/core";

import type { EntryChangesRequest } from "./generated/EntryChangesRequest";
import type { EntryView } from "./generated/EntryView";
import type { IpcError } from "./generated/IpcError";
import type { IpcErrorCode } from "./generated/IpcErrorCode";
import type { NewEntryRequest } from "./generated/NewEntryRequest";
import type { RevealedSecret } from "./generated/RevealedSecret";
import type { VaultStatus } from "./generated/VaultStatus";

export type {
  CodeState,
} from "./generated/CodeState";
export type { EntryChangesRequest, EntryView, IpcError, IpcErrorCode, NewEntryRequest };
export type { RevealedSecret, VaultStatus };

/** What a rejected command call becomes; `code` is the stable, switchable part. */
export class IpcCallError extends Error {
  readonly code: IpcErrorCode;

  constructor(error: IpcError) {
    super(error.message ?? error.code);
    this.name = "IpcCallError";
    this.code = error.code;
  }
}

function isIpcError(value: unknown): value is IpcError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { code?: unknown }).code === "string"
  );
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (isIpcError(error)) {
      throw new IpcCallError(error);
    }
    throw error;
  }
}

export const ipc = {
  vaultStatus: () => call<VaultStatus>("vault_status"),
  createVault: (password: string) => call<void>("create_vault", { password }),
  unlock: (password: string) => call<void>("unlock", { password }),
  lock: () => call<void>("lock"),
  recordActivity: () => call<void>("record_activity"),
  listEntries: () => call<EntryView[]>("list_entries"),
  addEntry: (request: NewEntryRequest) => call<string>("add_entry", { request }),
  editEntry: (id: string, request: EntryChangesRequest) =>
    call<void>("edit_entry", { id, request }),
  deleteEntry: (id: string) => call<void>("delete_entry", { id }),
  moveEntry: (id: string, position: number) => call<void>("move_entry", { id, position }),
  advanceCounter: (id: string) => call<void>("advance_counter", { id }),
  revealSecret: (id: string, password: string) =>
    call<RevealedSecret>("reveal_secret", { id, password }),
  changePassword: (current: string, replacement: string) =>
    call<void>("change_password", { current, replacement }),
};
