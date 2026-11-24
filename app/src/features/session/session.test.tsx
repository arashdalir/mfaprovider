import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiContext, type VaultApi } from "@/ipc/api-context";
import { IpcCallError } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { CreateVaultScreen } from "./CreateVaultScreen";
import { SessionGate } from "./SessionGate";
import { UnlockScreen } from "./UnlockScreen";

function withApi(api: VaultApi, ui: React.ReactNode) {
  return <ApiContext.Provider value={api}>{ui}</ApiContext.Provider>;
}

afterEach(() => {
  vi.useRealTimers();
});

describe("CreateVaultScreen", () => {
  it("does not call the core while the passwords are unacceptable", async () => {
    const api = fakeApi();
    render(withApi(api, <CreateVaultScreen onCreated={vi.fn()} />));
    await userEvent.type(screen.getByLabelText("Master password"), "short");
    await userEvent.click(screen.getByRole("button", { name: "Create vault" }));
    expect(screen.getByRole("alert")).toHaveTextContent(/at least 8/);
    expect(api.createVault).not.toHaveBeenCalled();
  });

  it("flags a mismatching repeat", async () => {
    const api = fakeApi();
    render(withApi(api, <CreateVaultScreen onCreated={vi.fn()} />));
    await userEvent.type(screen.getByLabelText("Master password"), "long enough");
    await userEvent.type(screen.getByLabelText("Repeat master password"), "long enough!");
    await userEvent.click(screen.getByRole("button", { name: "Create vault" }));
    expect(screen.getByRole("alert")).toHaveTextContent(/do not match/);
    expect(api.createVault).not.toHaveBeenCalled();
  });

  it("creates the vault and reports it", async () => {
    const api = fakeApi();
    const onCreated = vi.fn();
    render(withApi(api, <CreateVaultScreen onCreated={onCreated} />));
    await userEvent.type(screen.getByLabelText("Master password"), "long enough");
    await userEvent.type(screen.getByLabelText("Repeat master password"), "long enough");
    await userEvent.click(screen.getByRole("button", { name: "Create vault" }));
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
    expect(api.createVault).toHaveBeenCalledWith("long enough");
  });

  it("shows the core's failure and stays usable", async () => {
    const api = fakeApi({
      createVault: vi.fn().mockRejectedValue(new IpcCallError({ code: "storage_failed", message: null })),
    });
    const onCreated = vi.fn();
    render(withApi(api, <CreateVaultScreen onCreated={onCreated} />));
    await userEvent.type(screen.getByLabelText("Master password"), "long enough");
    await userEvent.type(screen.getByLabelText("Repeat master password"), "long enough");
    await userEvent.click(screen.getByRole("button", { name: "Create vault" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be read or written/);
    expect(onCreated).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Create vault" })).toBeEnabled();
  });
});

describe("UnlockScreen", () => {
  it("unlocks with the typed password", async () => {
    const api = fakeApi();
    const onUnlocked = vi.fn();
    render(withApi(api, <UnlockScreen onUnlocked={onUnlocked} />));
    await userEvent.type(screen.getByLabelText("Master password"), "my password{enter}");
    await waitFor(() => expect(onUnlocked).toHaveBeenCalled());
    expect(api.unlock).toHaveBeenCalledWith("my password");
  });

  it("says so on a wrong password and clears the field", async () => {
    const api = fakeApi({
      unlock: vi.fn().mockRejectedValue(new IpcCallError({ code: "wrong_password", message: null })),
    });
    render(withApi(api, <UnlockScreen onUnlocked={vi.fn()} />));
    await userEvent.type(screen.getByLabelText("Master password"), "nope{enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Wrong password.");
    expect(screen.getByLabelText("Master password")).toHaveValue("");
  });

  it("cannot be submitted empty", () => {
    render(withApi(fakeApi(), <UnlockScreen onUnlocked={vi.fn()} />));
    expect(screen.getByRole("button", { name: "Unlock" })).toBeDisabled();
  });
});

describe("SessionGate", () => {
  const open = () => <p>the vault is open</p>;

  it("offers to create a vault when none exists", async () => {
    const api = fakeApi({ vaultStatus: vi.fn().mockResolvedValue({ exists: false, unlocked: false }) });
    render(withApi(api, <SessionGate>{open}</SessionGate>));
    expect(await screen.findByRole("heading", { name: "Create your vault" })).toBeInTheDocument();
  });

  it("asks for the password when a vault exists", async () => {
    render(withApi(fakeApi(), <SessionGate>{open}</SessionGate>));
    expect(await screen.findByRole("heading", { name: "Unlock" })).toBeInTheDocument();
  });

  it("shows the content once unlocked and locks on request", async () => {
    const status = vi
      .fn()
      .mockResolvedValueOnce({ exists: true, unlocked: true })
      .mockResolvedValue({ exists: true, unlocked: false });
    const api = fakeApi({ vaultStatus: status });
    render(
      withApi(
        api,
        <SessionGate>
          {({ lock }) => <button onClick={() => void lock()}>Lock now</button>}
        </SessionGate>,
      ),
    );
    await userEvent.click(await screen.findByRole("button", { name: "Lock now" }));
    expect(api.lock).toHaveBeenCalled();
    expect(await screen.findByRole("heading", { name: "Unlock" })).toBeInTheDocument();
  });

  it("returns to the unlock screen when the core locks itself", async () => {
    vi.useFakeTimers();
    const status = vi
      .fn()
      .mockResolvedValueOnce({ exists: true, unlocked: true })
      .mockResolvedValue({ exists: true, unlocked: false });
    render(withApi(fakeApi({ vaultStatus: status }), <SessionGate>{open}</SessionGate>));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(screen.getByText("the vault is open")).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(screen.getByRole("heading", { name: "Unlock" })).toBeInTheDocument();
  });

  it("reports a failing start and can retry", async () => {
    const status = vi
      .fn()
      .mockRejectedValueOnce(new IpcCallError({ code: "storage_failed", message: null }))
      .mockResolvedValue({ exists: true, unlocked: false });
    render(withApi(fakeApi({ vaultStatus: status }), <SessionGate>{open}</SessionGate>));
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be read or written/);
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Unlock" })).toBeInTheDocument();
  });

  it("keeps the open vault visible through a failed status poll", async () => {
    vi.useFakeTimers();
    const status = vi
      .fn()
      .mockResolvedValueOnce({ exists: true, unlocked: true })
      .mockRejectedValue(new Error("transient"));
    render(withApi(fakeApi({ vaultStatus: status }), <SessionGate>{open}</SessionGate>));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2500);
    });
    expect(screen.getByText("the vault is open")).toBeInTheDocument();
  });
});
