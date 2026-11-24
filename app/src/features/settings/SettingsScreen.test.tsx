import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiContext } from "@/ipc/api-context";
import { IpcCallError } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { SettingsScreen } from "./SettingsScreen";
import { loadSettings } from "./settings";

afterEach(() => {
  window.localStorage.clear();
});

function open(setIdleLock = vi.fn().mockResolvedValue(undefined)) {
  const api = fakeApi({ setIdleLock });
  const onThemeChange = vi.fn();
  const onClose = vi.fn();
  render(
    <ApiContext.Provider value={api}>
      <SettingsScreen theme="system" onThemeChange={onThemeChange} onClose={onClose} />
    </ApiContext.Provider>,
  );
  return { api, onThemeChange, onClose };
}

describe("SettingsScreen", () => {
  it("starts from the defaults", () => {
    open();
    expect(screen.getByLabelText("Lock automatically after")).toHaveValue("5");
    expect(screen.getByLabelText("Clear copied codes after")).toHaveValue("30");
  });

  it("applies and remembers a new idle time", async () => {
    const { api } = open();
    await userEvent.selectOptions(screen.getByLabelText("Lock automatically after"), "15");
    expect(api.setIdleLock).toHaveBeenCalledWith(900);
    expect(loadSettings().idleLockMinutes).toBe(15);
  });

  it("can switch the idle lock off", async () => {
    const { api } = open();
    await userEvent.selectOptions(screen.getByLabelText("Lock automatically after"), "never");
    expect(api.setIdleLock).toHaveBeenCalledWith(null);
    expect(loadSettings().idleLockMinutes).toBeNull();
  });

  it("remembers the clipboard delay", async () => {
    open();
    await userEvent.selectOptions(screen.getByLabelText("Clear copied codes after"), "60");
    expect(loadSettings().clipboardClearSeconds).toBe(60);
  });

  it("reports when the core refuses a change", async () => {
    open(vi.fn().mockRejectedValue(new IpcCallError({ code: "locked", message: null })));
    await userEvent.selectOptions(screen.getByLabelText("Lock automatically after"), "1");
    expect(await screen.findByRole("alert")).toHaveTextContent("The vault is locked.");
  });

  it("changes the theme and closes", async () => {
    const { onThemeChange, onClose } = open();
    await userEvent.selectOptions(screen.getByLabelText("Theme"), "dark");
    expect(onThemeChange).toHaveBeenCalledWith("dark");
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalled();
  });
});
