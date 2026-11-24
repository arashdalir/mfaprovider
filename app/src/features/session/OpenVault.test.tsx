import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ApiContext } from "@/ipc/api-context";
import type { EntryView } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { OpenVault } from "./OpenVault";

const entry: EntryView = {
  id: "1",
  otp_type: "totp",
  issuer: "GitHub",
  account: "alice",
  icon: null,
  tags: [],
  notes: "",
  params: {},
  created_at_ms: 0,
  updated_at_ms: 0,
  code: { state: "ready", text: "482913", period_seconds: 30, seconds_remaining: 12 },
};

function open(onLock = vi.fn()) {
  const api = fakeApi({
    listEntries: vi.fn().mockResolvedValue([entry]),
    listOtpTypes: vi.fn().mockResolvedValue([{ id: "totp", params: [] }]),
  });
  render(
    <ApiContext.Provider value={api}>
      <OpenVault onLock={onLock} theme="system" onThemeChange={vi.fn()} />
    </ApiContext.Provider>,
  );
  return { api, onLock };
}

describe("OpenVault", () => {
  it("shows the list with add and lock buttons", async () => {
    const { onLock } = open();
    expect(await screen.findByText(/GitHub/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Lock" }));
    expect(onLock).toHaveBeenCalled();
  });

  it("opens the add chooser and returns to the list on cancel", async () => {
    open();
    await userEvent.click(await screen.findByRole("button", { name: "Add account" }));
    expect(await screen.findByRole("region", { name: "Add account" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText(/GitHub/)).toBeInTheDocument();
  });

  it("opens the edit form for the chosen account and returns after saving", async () => {
    const { api } = open();
    await userEvent.click(await screen.findByRole("button", { name: "Edit GitHub · alice" }));
    expect(await screen.findByLabelText("Issuer")).toHaveValue("GitHub");
    await userEvent.type(screen.getByLabelText("Account"), "!");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.editEntry).toHaveBeenCalledWith("1", expect.objectContaining({ account: "alice!" }));
    expect(await screen.findByRole("button", { name: "Add account" })).toBeInTheDocument();
  });
});

describe("OpenVault settings", () => {
  it("tells the core the remembered idle time once unlocked", async () => {
    window.localStorage.setItem(
      "mfa-settings",
      JSON.stringify({ idleLockMinutes: 15, clipboardClearSeconds: 30 }),
    );
    const { api } = open();
    await screen.findByText(/GitHub/);
    expect(api.setIdleLock).toHaveBeenCalledWith(900);
    window.localStorage.clear();
  });

  it("opens and closes the settings screen", async () => {
    open();
    await userEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("region", { name: "Settings" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(await screen.findByText(/GitHub/)).toBeInTheDocument();
  });
});

describe("OpenVault import", () => {
  it("opens the import screen and returns to the list", async () => {
    open();
    await userEvent.click(await screen.findByRole("button", { name: "Import" }));
    expect(await screen.findByRole("region", { name: "Import" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByText(/GitHub/)).toBeInTheDocument();
  });
});
