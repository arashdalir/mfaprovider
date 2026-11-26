import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ApiContext, type VaultApi } from "@/ipc/api-context";
import {
  IpcCallError,
  type DraftView,
  type DuplicateView,
  type EntryView,
  type OtpTypeView,
} from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { EntryForm } from "./EntryForm";

const types: OtpTypeView[] = [
  {
    id: "hotp",
    params: [{ name: "counter", kind: { kind: "integer", min: 0, max: 4294967295 }, default: "0" }],
  },
  { id: "steam", params: [] },
  {
    id: "totp",
    params: [
      { name: "algorithm", kind: { kind: "choice", choices: ["SHA1", "SHA256", "SHA512"] }, default: "SHA1" },
      { name: "digits", kind: { kind: "integer", min: 6, max: 8 }, default: "6" },
      { name: "period", kind: { kind: "integer", min: 1, max: 3600 }, default: "30" },
    ],
  },
];

const entry: EntryView = {
  id: "abc",
  otp_type: "totp",
  issuer: "GitHub",
  account: "alice",
  icon: null,
  tags: ["work"],
  notes: "n",
  params: { digits: "8" },
  created_at_ms: 0,
  updated_at_ms: 0,
  code: { state: "unsupported" },
};

function open(props: { entry?: EntryView; draft?: DraftView }, overrides: Partial<VaultApi> = {}) {
  const api = fakeApi({ listOtpTypes: vi.fn().mockResolvedValue(types), ...overrides });
  const onDone = vi.fn();
  const onCancel = vi.fn();
  render(
    <ApiContext.Provider value={api}>
      <EntryForm {...props} onDone={onDone} onCancel={onCancel} />
    </ApiContext.Provider>,
  );
  return { api, onDone, onCancel };
}

describe("adding an account", () => {
  it("adds a TOTP account with schema defaults", async () => {
    const { api, onDone } = open({});
    await userEvent.type(await screen.findByLabelText("Issuer"), "GitHub");
    await userEvent.type(screen.getByLabelText("Account"), "alice");
    await userEvent.type(screen.getByLabelText("Secret key"), "JBSWY3DPEHPK3PXP");
    await userEvent.type(screen.getByLabelText(/Tags/), "work, mail");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(api.addEntry).toHaveBeenCalledWith({
      otp_type: "totp",
      issuer: "GitHub",
      account: "alice",
      secret_base32: "JBSWY3DPEHPK3PXP",
      params: { algorithm: "SHA1", digits: "6", period: "30" },
      icon: null,
      tags: ["work", "mail"],
      notes: "",
    });
    expect(onDone).toHaveBeenCalled();
  });

  it("generates the parameter fields from the chosen type", async () => {
    open({});
    await screen.findByLabelText("Issuer");
    expect(screen.getByLabelText("Period")).toHaveValue(30);
    await userEvent.selectOptions(screen.getByLabelText("Type"), "steam");
    expect(screen.queryByLabelText("Period")).not.toBeInTheDocument();
    await userEvent.selectOptions(screen.getByLabelText("Type"), "hotp");
    expect(screen.getByLabelText("Counter")).toHaveValue(0);
    expect(screen.queryByLabelText("Digits")).not.toBeInTheDocument();
  });

  it("does not call the core for an incomplete form", async () => {
    const { api } = open({});
    await screen.findByLabelText("Issuer");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("alert")).toHaveTextContent(/issuer or an account/);
    await userEvent.type(screen.getByLabelText("Account"), "me");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("alert")).toHaveTextContent(/secret key/);
    expect(api.addEntry).not.toHaveBeenCalled();
  });

  it("rejects parameters outside the schema's range", async () => {
    const { api } = open({});
    await userEvent.type(await screen.findByLabelText("Account"), "me");
    await userEvent.type(screen.getByLabelText("Secret key"), "JBSWY3DP");
    await userEvent.clear(screen.getByLabelText("Digits"));
    await userEvent.type(screen.getByLabelText("Digits"), "9");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("alert")).toHaveTextContent(/digits/);
    expect(api.addEntry).not.toHaveBeenCalled();
  });

  it("hides the secret until asked and shows the core's rejection", async () => {
    const { onDone } = open(
      {},
      { addEntry: vi.fn().mockRejectedValue(new IpcCallError({ code: "invalid_input", message: "the secret is not valid base32" })) },
    );
    const secret = await screen.findByLabelText("Secret key");
    expect(secret).toHaveAttribute("type", "password");
    await userEvent.type(secret, "not base32!");
    await userEvent.click(screen.getByRole("button", { name: "Show" }));
    expect(secret).toHaveAttribute("type", "text");
    await userEvent.type(screen.getByLabelText("Account"), "me");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("the secret is not valid base32");
    expect(onDone).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Add" })).toBeEnabled();
  });

  it("cancels without calling the core", async () => {
    const { api, onCancel } = open({});
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
    expect(api.addEntry).not.toHaveBeenCalled();
  });

  it("reports types that could not be loaded", async () => {
    open({}, { listOtpTypes: vi.fn().mockRejectedValue(new IpcCallError({ code: "storage_failed", message: null })) });
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be read or written/);
  });
});

describe("editing an account", () => {
  it("shows the current values and keeps the secret by default", async () => {
    open({ entry });
    expect(await screen.findByLabelText("Issuer")).toHaveValue("GitHub");
    expect(screen.getByLabelText("Digits")).toHaveValue(8);
    expect(screen.getByLabelText(/New secret key/)).toHaveValue("");
    expect(screen.queryByLabelText("Type")).not.toBeInTheDocument();
  });

  it("sends only what changed", async () => {
    const { api, onDone } = open({ entry });
    const account = await screen.findByLabelText("Account");
    await userEvent.clear(account);
    await userEvent.type(account, "bob");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.editEntry).toHaveBeenCalledWith("abc", {
      issuer: null,
      account: "bob",
      secret_base32: null,
      params: null,
      icon: null,
      clear_icon: false,
      tags: null,
      notes: null,
    });
    expect(onDone).toHaveBeenCalled();
  });

  it("replaces the secret when a new one is typed", async () => {
    const { api } = open({ entry });
    await userEvent.type(await screen.findByLabelText(/New secret key/), "JBSWY3DP");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.editEntry).toHaveBeenCalledWith("abc", expect.objectContaining({ secret_base32: "JBSWY3DP" }));
  });

  it("offers only metadata for an unsupported type and never sends parameters", async () => {
    const unknown: EntryView = { ...entry, otp_type: "vendor-x", params: { weird: "1" } };
    const { api } = open({ entry: unknown });
    await userEvent.type(await screen.findByLabelText("Issuer"), "!");
    expect(screen.getByText(/not supported by this version/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Digits")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.editEntry).toHaveBeenCalledWith("abc", expect.objectContaining({ issuer: "GitHub!", params: null }));
  });

  it("deletes only after an explicit confirmation", async () => {
    const { api, onDone } = open({ entry });
    await userEvent.click(await screen.findByRole("button", { name: "Delete…" }));
    expect(api.deleteEntry).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Delete…" }));
    await userEvent.click(screen.getByRole("button", { name: "Delete permanently" }));
    expect(api.deleteEntry).toHaveBeenCalledWith("abc");
    expect(onDone).toHaveBeenCalled();
  });

  it("keeps the form open when saving fails", async () => {
    const { onDone } = open(
      { entry },
      { editEntry: vi.fn().mockRejectedValue(new IpcCallError({ code: "storage_failed", message: null })) },
    );
    const issuer = await screen.findByLabelText("Issuer");
    await userEvent.type(issuer, "x");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be read or written/);
    expect(onDone).not.toHaveBeenCalled();
  });
});

describe("warning about an account that is already there", () => {
  const sameAccount: DuplicateView = { kind: "same_account", matches: "GitHub · alice" };

  async function fillManualForm() {
    await userEvent.type(await screen.findByLabelText("Issuer"), "GitHub");
    await userEvent.type(screen.getByLabelText("Account"), "alice");
    await userEvent.type(screen.getByLabelText("Secret key"), "JBSWY3DPEHPK3PXP");
  }

  it("adds straight away when nothing matches", async () => {
    const { api, onDone } = open({});
    await fillManualForm();
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(api.checkDuplicate).toHaveBeenCalledOnce();
    expect(api.addEntry).toHaveBeenCalledOnce();
    expect(onDone).toHaveBeenCalled();
  });

  it("warns instead of adding, and adds only after a second, deliberate press", async () => {
    const { api, onDone } = open({}, { checkDuplicate: vi.fn().mockResolvedValue(sameAccount) });
    await fillManualForm();
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("status")).toHaveTextContent("This account is already in your vault: GitHub · alice.");
    expect(api.addEntry).not.toHaveBeenCalled();
    expect(onDone).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Add anyway" }));
    expect(api.addEntry).toHaveBeenCalledOnce();
    expect(onDone).toHaveBeenCalled();
  });

  it("says what kind of match it is", async () => {
    open(
      {},
      {
        checkDuplicate: vi
          .fn()
          .mockResolvedValue({ kind: "same_key_other_name", matches: "GitHub · work" }),
      },
    );
    await fillManualForm();
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "This secret key is already in your vault under another name: GitHub · work.",
    );
  });

  it("drops the warning when a field is edited, and checks again on the next add", async () => {
    const checkDuplicate = vi.fn().mockResolvedValueOnce(sameAccount).mockResolvedValue(null);
    const { api } = open({}, { checkDuplicate });
    await fillManualForm();
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    await userEvent.type(screen.getByLabelText("Account"), "2");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(checkDuplicate).toHaveBeenCalledTimes(2);
    expect(api.addEntry).toHaveBeenCalledOnce();
  });

  it("does not add when the check itself fails", async () => {
    const { api } = open(
      {},
      { checkDuplicate: vi.fn().mockRejectedValue(new IpcCallError({ code: "locked", message: null })) },
    );
    await fillManualForm();
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(api.addEntry).not.toHaveBeenCalled();
  });

  it("warns as soon as a scanned or pasted account opens, and adds it after Add anyway", async () => {
    const draft: DraftView = {
      draft_id: 0,
      otp_type: "totp",
      issuer: "GitHub",
      account: "alice",
      params: {},
    };
    const { api, onDone } = open(
      { draft },
      { checkDraftDuplicate: vi.fn().mockResolvedValue(sameAccount) },
    );
    expect(await screen.findByRole("status")).toHaveTextContent("already in your vault");
    await userEvent.click(screen.getByRole("button", { name: "Add anyway" }));
    expect(api.addDraft).toHaveBeenCalledWith(0, expect.objectContaining({ account: "alice" }));
    expect(onDone).toHaveBeenCalled();
  });

  it("shows no warning when editing an existing account", async () => {
    open({ entry }, { checkDuplicate: vi.fn().mockResolvedValue(sameAccount) });
    await screen.findByLabelText("Issuer");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
