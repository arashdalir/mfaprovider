// The only module that talks to the Rust core (ADR-0012). Everything the UI may ask for is a
// typed function here; `no-restricted-imports` stops other files importing Tauri's `invoke`.
import { invoke } from "@tauri-apps/api/core";

import type { DraftEditsRequest } from "./generated/DraftEditsRequest";
import type { DraftView } from "./generated/DraftView";
import type { DuplicateView } from "./generated/DuplicateView";
import type { EntryChangesRequest } from "./generated/EntryChangesRequest";
import type { EntryView } from "./generated/EntryView";
import type { ImportChoice } from "./generated/ImportChoice";
import type { ImportDecision } from "./generated/ImportDecision";
import type { ImportFormatView } from "./generated/ImportFormatView";
import type { ImportItemView } from "./generated/ImportItemView";
import type { ImportPreview } from "./generated/ImportPreview";
import type { ImportResult } from "./generated/ImportResult";
import type { ImportStatus } from "./generated/ImportStatus";
import type { IpcError } from "./generated/IpcError";
import type { IpcErrorCode } from "./generated/IpcErrorCode";
import type { NewEntryRequest } from "./generated/NewEntryRequest";
import type { OtpTypeView } from "./generated/OtpTypeView";
import type { RevealedSecret } from "./generated/RevealedSecret";
import type { VaultStatus } from "./generated/VaultStatus";

export type {
  CodeState,
} from "./generated/CodeState";
export type { DraftEditsRequest, DraftView };
export type {
  ImportChoice,
  ImportDecision,
  ImportFormatView,
  ImportItemView,
  ImportPreview,
  ImportResult,
  ImportStatus,
};
export type { EntryChangesRequest, EntryView, IpcError, IpcErrorCode, NewEntryRequest };
export type { DuplicateView, RevealedSecret, VaultStatus };
export type { OtpTypeView };
export type { ParamKindView } from "./generated/ParamKindView";
export type { ParamSpecView } from "./generated/ParamSpecView";

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
  listOtpTypes: () => call<OtpTypeView[]>("list_otp_types"),
  createVault: (password: string) => call<void>("create_vault", { password }),
  unlock: (password: string) => call<void>("unlock", { password }),
  lock: () => call<void>("lock"),
  recordActivity: () => call<void>("record_activity"),
  setIdleLock: (seconds: number | null) => call<void>("set_idle_lock", { seconds }),
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
  draftFromLink: (text: string) => call<DraftView[]>("draft_from_link", { text }),
  /** `image` is the file's bytes as base64. */
  draftFromImage: (image: string) => call<DraftView[]>("draft_from_image", { image }),
  addDraft: (id: number, edits: DraftEditsRequest) => call<string>("add_draft", { id, edits }),
  /** Whether the account about to be added is already in the vault (null = no). */
  checkDuplicate: (request: NewEntryRequest) =>
    call<DuplicateView | null>("check_duplicate", { request }),
  checkDraftDuplicate: (id: number, edits: DraftEditsRequest) =>
    call<DuplicateView | null>("check_draft_duplicate", { id, edits }),
  discardDrafts: () => call<void>("discard_drafts"),
  listImportFormats: () => call<ImportFormatView[]>("list_import_formats"),
  /**
   * `file` is a text export or a ZIP archive as base64; an archive needs its `password`.
   * Nothing in the vault changes until `importApply`.
   */
  importPreview: (format: string, file: string, password: string | null = null) =>
    call<ImportPreview>("import_preview", { format, file, password }),
  importApply: (choices: ImportChoice[]) => call<ImportResult>("import_apply", { choices }),
  importDiscard: () => call<void>("import_discard"),
};
