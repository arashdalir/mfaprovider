import { beforeEach, describe, expect, it, vi } from "vitest";

import { IpcCallError, ipc } from "./client";
import clientSource from "./client.ts?raw";
import rustCommands from "../../src-tauri/src/ipc/commands.rs?raw";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

function namesIn(source: string, pattern: RegExp): string[] {
  return [...source.matchAll(pattern)].map((match) => match[1]).sort();
}

describe("command names", () => {
  it("the client calls exactly the commands the Rust side defines", () => {
    const called = namesIn(clientSource, /\bcall(?:<[^(]*>)?\(\s*"([a-z_]+)"/g);
    const defined = namesIn(rustCommands, /#\[tauri::command\]\s+pub async fn ([a-z_]+)\(/g);
    expect(defined.length).toBeGreaterThan(10);
    expect(called).toEqual(defined);
  });
});

describe("client calls", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
  });

  it("passes arguments under the names the Rust commands declare", async () => {
    const request = {
      issuer: "I",
      account: null,
      secret_base32: null,
      params: null,
      icon: null,
      clear_icon: false,
      tags: null,
      notes: null,
    };
    await ipc.unlock("pw");
    await ipc.createVault("pw");
    await ipc.editEntry("id-1", request);
    await ipc.moveEntry("id-1", 2);
    await ipc.revealSecret("id-1", "pw");
    await ipc.changePassword("old", "new");
    await ipc.deleteEntry("id-1");
    await ipc.advanceCounter("id-1");
    expect(invoke.mock.calls).toEqual([
      ["unlock", { password: "pw" }],
      ["create_vault", { password: "pw" }],
      ["edit_entry", { id: "id-1", request }],
      ["move_entry", { id: "id-1", position: 2 }],
      ["reveal_secret", { id: "id-1", password: "pw" }],
      ["change_password", { current: "old", replacement: "new" }],
      ["delete_entry", { id: "id-1" }],
      ["advance_counter", { id: "id-1" }],
    ]);
  });

  it("calls argument-less commands without arguments", async () => {
    await ipc.lock();
    await ipc.listEntries();
    await ipc.vaultStatus();
    await ipc.recordActivity();
    expect(invoke.mock.calls.map((call) => call[0])).toEqual([
      "lock",
      "list_entries",
      "vault_status",
      "record_activity",
    ]);
    expect(invoke.mock.calls.every((call) => call[1] === undefined)).toBe(true);
  });

  it("returns what the command returns", async () => {
    invoke.mockResolvedValue({ exists: true, unlocked: false });
    await expect(ipc.vaultStatus()).resolves.toEqual({ exists: true, unlocked: false });
  });

  it("turns a rejected IpcError into an IpcCallError with its code", async () => {
    invoke.mockRejectedValue({ code: "wrong_password", message: null });
    const failure = await ipc.unlock("x").catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(IpcCallError);
    expect((failure as IpcCallError).code).toBe("wrong_password");
  });

  it("keeps the safe message when there is one", async () => {
    invoke.mockRejectedValue({ code: "invalid_input", message: "the secret is empty" });
    await expect(ipc.addEntry({} as never)).rejects.toThrow("the secret is empty");
  });

  it("does not disguise unexpected failures as IPC errors", async () => {
    invoke.mockRejectedValue(new Error("webview gone"));
    const failure = await ipc.lock().catch((error: unknown) => error);
    expect(failure).not.toBeInstanceOf(IpcCallError);
    expect((failure as Error).message).toBe("webview gone");
  });
});
