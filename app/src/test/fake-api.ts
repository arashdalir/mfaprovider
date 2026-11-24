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
    revealSecret: vi.fn().mockResolvedValue({ secret_base32: "" }),
    changePassword: resolved(),
    ...overrides,
  };
}
