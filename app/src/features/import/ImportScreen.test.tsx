import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ApiContext, type VaultApi } from "@/ipc/api-context";
import { IpcCallError, type ImportItemView, type ImportPreview } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { ImportScreen } from "./ImportScreen";

function item(overrides: Partial<ImportItemView> & { item_id: number }): ImportItemView {
  return {
    position: overrides.item_id + 1,
    otp_type: "totp",
    issuer: "",
    account: `account${overrides.item_id}`,
    status: "new",
    matches: null,
    can_replace: false,
    default_decision: "add",
    ...overrides,
  };
}

function preview(items: ImportItemView[], skipped: ImportPreview["skipped"] = []): ImportPreview {
  return { items, skipped };
}

function open(overrides: Partial<VaultApi> = {}) {
  const api = fakeApi(overrides);
  const onClose = vi.fn();
  render(
    <ApiContext.Provider value={api}>
      <ImportScreen onClose={onClose} />
    </ApiContext.Provider>,
  );
  return { api, onClose };
}

const exportFile = (text = "otpauth://totp/a?secret=ABC\r\n") =>
  new File([text], "export.txt", { type: "text/plain" });

async function pick(file = exportFile()) {
  await userEvent.upload(await screen.findByLabelText("Choose the export file"), file);
}

describe("choosing the file", () => {
  it("offers the formats and sends the chosen file to the core as base64", async () => {
    const { api } = open({
      importPreview: vi.fn().mockResolvedValue(preview([item({ item_id: 0 })])),
    });
    expect(await screen.findByRole("option", { name: /otpauth/ })).toBeInTheDocument();
    await pick(exportFile("line one\r\nline two"));
    expect(api.importPreview).toHaveBeenCalledWith(
      "otpauth-list",
      btoa("line one\r\nline two"),
      null,
    );
    expect(await screen.findByRole("region", { name: "Import preview" })).toBeInTheDocument();
  });

  it("shows why a file could not be previewed and stays on the step", async () => {
    open({
      importPreview: vi
        .fn()
        .mockRejectedValue(new IpcCallError({ code: "invalid_input", message: "no accounts were found in the file" })),
    });
    await pick();
    expect(await screen.findByRole("alert")).toHaveTextContent("no accounts were found in the file");
    expect(screen.getByLabelText("Choose the export file")).toBeEnabled();
  });

  it("refuses files over 5 MiB without calling the core", async () => {
    const { api } = open();
    await pick(new File([new Uint8Array(5 * 1024 * 1024 + 1)], "huge.txt"));
    expect(await screen.findByRole("alert")).toHaveTextContent("larger than 5 MiB");
    expect(api.importPreview).not.toHaveBeenCalled();
  });

  it("lets the file picker offer text, JSON and ZIP files", async () => {
    open();
    const accept = (await screen.findByLabelText("Choose the export file")).getAttribute("accept");
    expect(accept?.split(",")).toEqual(expect.arrayContaining([".txt", ".json", ".zip"]));
  });

});

describe("a password-protected archive", () => {
  const needsPassword = () => new IpcCallError({ code: "password_required", message: null });
  const wrongPassword = () =>
    new IpcCallError({ code: "invalid_input", message: "wrong password, or the file is damaged" });
  const zip = () => new File([new Uint8Array([0x50, 0x4b, 3, 4])], "export.zip");

  it("asks for the password, then opens the file with it", async () => {
    const importPreview = vi
      .fn()
      .mockRejectedValueOnce(needsPassword())
      .mockResolvedValue(preview([item({ item_id: 0 })]));
    open({ importPreview });
    await pick(zip());
    expect(await screen.findByRole("alert")).toHaveTextContent("This file needs a password.");
    await userEvent.type(screen.getByLabelText("Password of the ZIP file"), "hunter2");
    await userEvent.click(screen.getByRole("button", { name: "Open file" }));
    expect(importPreview).toHaveBeenLastCalledWith("otpauth-list", expect.any(String), "hunter2");
    expect(await screen.findByRole("region", { name: "Import preview" })).toBeInTheDocument();
  });

  it("keeps asking after a wrong password, with the reason", async () => {
    const importPreview = vi
      .fn()
      .mockRejectedValueOnce(needsPassword())
      .mockRejectedValue(wrongPassword());
    open({ importPreview });
    await pick(zip());
    await userEvent.type(await screen.findByLabelText("Password of the ZIP file"), "nope{enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("wrong password, or the file is damaged");
    expect(screen.getByLabelText("Password of the ZIP file")).toBeEnabled();
  });

  it("does not offer to open the file before a password is typed", async () => {
    open({ importPreview: vi.fn().mockRejectedValue(needsPassword()) });
    await pick(zip());
    await screen.findByLabelText("Password of the ZIP file");
    expect(screen.getByRole("button", { name: "Open file" })).toBeDisabled();
  });

  it("forgets the password and the file when another file is chosen", async () => {
    const importPreview = vi.fn().mockRejectedValueOnce(needsPassword()).mockResolvedValue(preview([]));
    open({ importPreview });
    await pick(zip());
    await screen.findByLabelText("Password of the ZIP file");
    await pick(exportFile());
    expect(screen.queryByLabelText("Password of the ZIP file")).not.toBeInTheDocument();
    expect(importPreview).toHaveBeenLastCalledWith("otpauth-list", expect.any(String), null);
  });
});

describe("reviewing and applying", () => {
  const items = [
    item({ item_id: 0, account: "fresh" }),
    item({ item_id: 1, account: "same", issuer: "Example", status: "duplicate_in_vault", matches: "Example · same", can_replace: true, default_decision: "skip" }),
    item({ item_id: 2, account: "clash", status: "conflict", matches: "clash", can_replace: true, default_decision: "add" }),
    item({ item_id: 3, account: "twice", status: "duplicate_in_file", matches: "line 1", default_decision: "skip" }),
  ];

  async function review(overrides: Partial<VaultApi> = {}, list = items) {
    const opened = open({
      importPreview: vi.fn().mockResolvedValue(preview(list, [{ position: 9, reason: "this is not an otpauth:// link" }])),
      ...overrides,
    });
    await pick();
    await screen.findByRole("region", { name: "Import preview" });
    return opened;
  }

  it("starts from the suggested choices and says what they add up to", async () => {
    await review();
    expect(screen.getByText(/4 accounts found, 1 lines could not be read/)).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Will add 2, replace 0, skip 2.");
    expect(screen.getByRole("button", { name: "Import 2 accounts" })).toBeEnabled();
    expect(screen.getByLabelText("Import fresh")).toBeChecked();
    expect(screen.getByLabelText("Action for Example · same")).toHaveValue("skip");
    expect(screen.getByLabelText("Action for clash")).toHaveValue("add");
  });

  it("lists the lines that could not be read, with the reason", async () => {
    await review();
    expect(screen.getByText("Line 9: this is not an otpauth:// link")).toBeInTheDocument();
  });

  it("describes each account's status and what it matches", async () => {
    await review();
    expect(screen.getByText("Already in the vault (Example · same)")).toBeInTheDocument();
    expect(screen.getByText("Same name, different key (clash)")).toBeInTheDocument();
    expect(screen.getByText("Repeated in this file (line 1)")).toBeInTheDocument();
  });

  it("offers replacing only where there is something to replace", async () => {
    await review();
    const options = (name: string) =>
      [...(screen.getByLabelText(name) as HTMLSelectElement).options].map((o) => o.value);
    expect(options("Action for clash")).toEqual(["skip", "add", "replace"]);
    expect(options("Action for twice")).toEqual(["skip", "add"]);
  });

  it("applies exactly the choices made", async () => {
    const { api } = await review();
    await userEvent.click(screen.getByLabelText("Import fresh"));
    await userEvent.selectOptions(screen.getByLabelText("Action for clash"), "replace");
    await userEvent.selectOptions(screen.getByLabelText("Action for Example · same"), "add");
    expect(screen.getByRole("status")).toHaveTextContent("Will add 1, replace 1, skip 2.");
    await userEvent.click(screen.getByRole("button", { name: "Import 2 accounts" }));
    expect(api.importApply).toHaveBeenCalledWith([
      { item_id: 0, decision: "skip" },
      { item_id: 1, decision: "add" },
      { item_id: 2, decision: "replace" },
      { item_id: 3, decision: "skip" },
    ]);
  });

  it("can skip everything and go back to the suggestions", async () => {
    await review();
    await userEvent.click(screen.getByRole("button", { name: "Skip all" }));
    expect(screen.getByRole("status")).toHaveTextContent("Will add 0, replace 0, skip 4.");
    expect(screen.getByRole("button", { name: "Import 0 accounts" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Suggested choices" }));
    expect(screen.getByRole("status")).toHaveTextContent("Will add 2, replace 0, skip 2.");
  });

  it("reports the result and closes from the finished screen", async () => {
    const { onClose } = await review({
      importApply: vi.fn().mockResolvedValue({ added: 2, replaced: 0, skipped: 2 }),
    });
    await userEvent.click(screen.getByRole("button", { name: "Import 2 accounts" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Added 2, replaced 0, skipped 2.");
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("keeps the review open and explains when applying fails", async () => {
    const { onClose } = await review({
      importApply: vi.fn().mockRejectedValue(new IpcCallError({ code: "storage_failed", message: null })),
    });
    await userEvent.click(screen.getByRole("button", { name: "Import 2 accounts" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be read or written/);
    expect(screen.getByRole("region", { name: "Import preview" })).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("discards the pending file when cancelled", async () => {
    const { api, onClose } = await review();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(api.importDiscard).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("shows a file with nothing usable as a report that cannot be applied", async () => {
    await review({ importPreview: vi.fn().mockResolvedValue(preview([], [{ position: 1, reason: "the link is malformed" }])) }, []);
    expect(screen.getByText(/0 accounts found, 1 lines could not be read/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Import 0 accounts" })).toBeDisabled();
  });
});
