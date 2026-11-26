import { vi } from "vitest";

import type { VaultApi } from "@/ipc/api-context";

/** An API whose every call resolves to nothing, unless overridden; calls are recorded by vi.fn. */
export function fakeApi(overrides: Partial<VaultApi> = {}): VaultApi {
  const resolved = () => vi.fn().mockResolvedValue(undefined);
  return {
    vaultStatus: vi.fn().mockResolvedValue({ exists: true, unlocked: false }),
    listOtpTypes: vi.fn().mockResolvedValue([]),
    createVault: resolved(),
    unlock: resolved(),
    lock: resolved(),
    recordActivity: resolved(),
    setIdleLock: resolved(),
    listEntries: vi.fn().mockResolvedValue([]),
    addEntry: vi.fn().mockResolvedValue("new-id"),
    editEntry: resolved(),
    deleteEntry: resolved(),
    moveEntry: resolved(),
    advanceCounter: resolved(),
    revealSecret: vi.fn().mockResolvedValue({ secret_base32: "", qr_svg: null }),
    changePassword: resolved(),
    draftFromLink: vi.fn().mockResolvedValue([]),
    draftFromImage: vi.fn().mockResolvedValue([]),
    addDraft: vi.fn().mockResolvedValue("new-id"),
    reportBackgrounded: vi.fn().mockResolvedValue(undefined),
    reportForegrounded: vi.fn().mockResolvedValue(undefined),
    expectFilePicker: vi.fn().mockResolvedValue(undefined),
    biometricStatus: vi
      .fn()
      .mockResolvedValue({ available: false, enabled: false, password_due: false, reprompt_days: 7 }),
    biometricEnable: vi.fn().mockResolvedValue(undefined),
    biometricDisable: vi.fn().mockResolvedValue(undefined),
    unlockWithBiometric: vi.fn().mockResolvedValue(undefined),
    setBiometricRepromptDays: vi.fn().mockResolvedValue(undefined),
    checkDuplicate: vi.fn().mockResolvedValue(null),
    checkDraftDuplicate: vi.fn().mockResolvedValue(null),
    discardDrafts: resolved(),
    listImportFormats: vi
      .fn()
      .mockResolvedValue([{ id: "otpauth-list", label: "Text file of otpauth:// links" }]),
    importPreview: vi.fn().mockResolvedValue({ items: [], skipped: [] }),
    importApply: vi.fn().mockResolvedValue({ added: 0, replaced: 0, skipped: 0 }),
    importDiscard: resolved(),
    ...overrides,
  };
}
