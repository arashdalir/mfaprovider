import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiContext } from "@/ipc/api-context";
import { IpcCallError, type BiometricStatus } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";
import { devicePasswordName } from "@/features/credentials/credential-names";

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

describe("SettingsScreen biometrics", () => {
  const status = (over: object) =>
    vi.fn().mockResolvedValue({ available: true, enabled: false, password_due: false, reprompt_days: 7, ...over });

  function openWith(biometricStatus: () => Promise<BiometricStatus>, extra = {}) {
    const api = fakeApi({ biometricStatus, ...extra });
    render(
      <ApiContext.Provider value={api}>
        <SettingsScreen theme="system" onThemeChange={vi.fn()} onClose={vi.fn()} />
      </ApiContext.Provider>,
    );
    return api;
  }

  it("shows no switch where the device has no biometrics", async () => {
    const api = openWith(status({ available: false }));
    await waitFor(() => expect(api.biometricStatus).toHaveBeenCalled());
    expect(screen.queryByLabelText("Unlock with fingerprint or face")).not.toBeInTheDocument();
  });

  it("turns it on only after the device password is typed", async () => {
    const api = openWith(status({}));
    await userEvent.click(await screen.findByLabelText("Unlock with fingerprint or face"));
    expect(api.biometricEnable).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText("Device password"), "pw");
    await userEvent.click(screen.getByRole("button", { name: "Turn on" }));
    expect(api.biometricEnable).toHaveBeenCalledWith("pw");
  });

  it("asks for it under the device password's name in password managers", async () => {
    openWith(status({}));
    await userEvent.click(await screen.findByLabelText("Unlock with fingerprint or face"));
    const password = screen.getByLabelText<HTMLInputElement>("Device password");
    expect(password.form?.querySelector('input[autocomplete="username"]')).toHaveValue(devicePasswordName());
  });

  it("says so when the password is wrong and keeps it off", async () => {
    openWith(status({}), {
      biometricEnable: vi.fn().mockRejectedValue(new IpcCallError({ code: "wrong_password", message: null })),
    });
    await userEvent.click(await screen.findByLabelText("Unlock with fingerprint or face"));
    await userEvent.type(screen.getByLabelText("Device password"), "nope{enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Wrong password.");
    expect(screen.getByLabelText("Device password")).toHaveValue("");
  });

  it("turns it off and changes the re-prompt period", async () => {
    const api = openWith(status({ enabled: true }));
    await userEvent.selectOptions(await screen.findByLabelText("Ask for the password again after"), "30");
    expect(api.setBiometricRepromptDays).toHaveBeenCalledWith(30);
    await userEvent.click(screen.getByLabelText("Unlock with fingerprint or face"));
    expect(api.biometricDisable).toHaveBeenCalled();
  });
});

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

  it("hides codes by default and remembers turning that off", async () => {
    open();
    const box = screen.getByLabelText("Hide codes until I press Show");
    expect(box).toBeChecked();
    await userEvent.click(box);
    expect(loadSettings().hideCodes).toBe(false);
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
