import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";

import { RevealSecret } from "@/features/entries/RevealSecret";
import { CreateVaultScreen } from "@/features/session/CreateVaultScreen";
import { UnlockScreen } from "@/features/session/UnlockScreen";
import { ApiContext } from "@/ipc/api-context";
import { fakeApi } from "@/test/fake-api";

import { devicePasswordName } from "./credential-names";

function show(screenUnderTest: ReactElement) {
  render(<ApiContext.Provider value={fakeApi()}>{screenUnderTest}</ApiContext.Provider>);
}

/** What a password manager reads: the form around a password box and the user name in it. */
function loginAround(label: string) {
  const password = screen.getByLabelText<HTMLInputElement>(label);
  const form = password.form;
  const username = form?.querySelector<HTMLInputElement>('input[autocomplete="username"]');
  return { password, form, username };
}

describe("forms that take the device password", () => {
  it("offers a new vault's password for saving under the device password's name", () => {
    show(<CreateVaultScreen onCreated={vi.fn()} />);
    const { password, username } = loginAround("Device password");
    expect(username).toHaveValue(devicePasswordName());
    expect(username).toHaveAttribute("readonly");
    expect(password).toHaveAttribute("autocomplete", "new-password");
    expect(password).toHaveAttribute("name", "new-device-password");
    expect(screen.getByLabelText("Repeat device password")).toHaveAttribute("autocomplete", "new-password");
  });

  it("asks for it under the same name when unlocking", () => {
    show(<UnlockScreen onUnlocked={vi.fn()} />);
    const { password, username } = loginAround("Device password");
    expect(username).toHaveValue(devicePasswordName());
    expect(password).toHaveAttribute("autocomplete", "current-password");
    expect(password).toHaveAttribute("name", "device-password");
  });

  it("asks for it under the same name before showing an account's key", async () => {
    show(<RevealSecret entryId="abc" />);
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    const { password, username } = loginAround("Device password");
    expect(username).toHaveValue(devicePasswordName());
    expect(password).toHaveAttribute("name", "device-password");
  });
});
