import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ApiContext, type VaultApi } from "@/ipc/api-context";
import { IpcCallError, type DraftView } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { AddAccountScreen } from "./AddAccountScreen";

const totpType = {
  id: "totp",
  params: [{ name: "digits", kind: { kind: "integer" as const, min: 6, max: 8 }, default: "6" }],
};

const alice: DraftView = {
  draft_id: 0,
  otp_type: "totp",
  issuer: "Example",
  account: "alice",
  params: { digits: "8" },
};
const bob: DraftView = { ...alice, draft_id: 1, account: "bob" };

function open(overrides: Partial<VaultApi> = {}) {
  const api = fakeApi({ listOtpTypes: vi.fn().mockResolvedValue([totpType]), ...overrides });
  const onDone = vi.fn();
  const onCancel = vi.fn();
  render(
    <ApiContext.Provider value={api}>
      <AddAccountScreen onDone={onDone} onCancel={onCancel} />
    </ApiContext.Provider>,
  );
  return { api, onDone, onCancel };
}

function pngFile(size = 10) {
  return new File([new Uint8Array(size).fill(1)], "code.png", { type: "image/png" });
}

describe("choosing how to add", () => {
  it("offers the three ways and can be cancelled", async () => {
    const { onCancel } = open();
    expect(screen.getByRole("button", { name: "Enter details manually" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Paste an otpauth:// link" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Read a QR code from an image" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("manual entry opens the plain form and can go back", async () => {
    open();
    await userEvent.click(screen.getByRole("button", { name: "Enter details manually" }));
    expect(await screen.findByLabelText("Secret key")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("region", { name: "Add account" })).toBeInTheDocument();
  });
});

describe("pasting a link", () => {
  it("reads the link, shows the draft without any secret field, and adds it with the edits", async () => {
    const { api, onDone } = open({ draftFromLink: vi.fn().mockResolvedValue([alice]) });
    await userEvent.click(screen.getByRole("button", { name: "Paste an otpauth:// link" }));
    await userEvent.type(screen.getByLabelText("The otpauth:// link"), "otpauth://totp/x?secret=ABC");
    await userEvent.click(screen.getByRole("button", { name: "Read link" }));
    expect(api.draftFromLink).toHaveBeenCalledWith("otpauth://totp/x?secret=ABC");

    expect(await screen.findByLabelText("Issuer")).toHaveValue("Example");
    expect(screen.getByLabelText("Account")).toHaveValue("alice");
    expect(screen.getByLabelText("Digits")).toHaveValue(8);
    expect(screen.queryByLabelText(/secret key/i)).not.toBeInTheDocument();
    expect(screen.getByText(/stays inside the app/)).toBeInTheDocument();

    const account = screen.getByLabelText("Account");
    await userEvent.clear(account);
    await userEvent.type(account, "alice at work");
    await userEvent.type(screen.getByLabelText(/Tags/), "work");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(api.addDraft).toHaveBeenCalledWith(0, {
      issuer: "Example",
      account: "alice at work",
      params: { digits: "8" },
      tags: ["work"],
      notes: "",
    });
    expect(onDone).toHaveBeenCalled();
  });

  it("shows the core's complaint about a bad link and stays on the step", async () => {
    open({
      draftFromLink: vi
        .fn()
        .mockRejectedValue(new IpcCallError({ code: "invalid_input", message: "the link has no secret key" })),
    });
    await userEvent.click(screen.getByRole("button", { name: "Paste an otpauth:// link" }));
    expect(screen.getByRole("button", { name: "Read link" })).toBeDisabled();
    await userEvent.type(screen.getByLabelText("The otpauth:// link"), "otpauth://totp/x");
    await userEvent.click(screen.getByRole("button", { name: "Read link" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("the link has no secret key");
  });

  it("discards the draft when the review is cancelled", async () => {
    const { api } = open({ draftFromLink: vi.fn().mockResolvedValue([alice]) });
    await userEvent.click(screen.getByRole("button", { name: "Paste an otpauth:// link" }));
    await userEvent.type(screen.getByLabelText("The otpauth:// link"), "otpauth://totp/x?secret=ABC");
    await userEvent.click(screen.getByRole("button", { name: "Read link" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(api.discardDrafts).toHaveBeenCalled();
    expect(screen.getByRole("region", { name: "Add account" })).toBeInTheDocument();
  });

  it("keeps the review open when the core rejects the edits", async () => {
    const { onDone } = open({
      draftFromLink: vi.fn().mockResolvedValue([alice]),
      addDraft: vi.fn().mockRejectedValue(new IpcCallError({ code: "storage_failed", message: null })),
    });
    await userEvent.click(screen.getByRole("button", { name: "Paste an otpauth:// link" }));
    await userEvent.type(screen.getByLabelText("The otpauth:// link"), "otpauth://totp/x?secret=ABC");
    await userEvent.click(screen.getByRole("button", { name: "Read link" }));
    await userEvent.click(await screen.findByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be read or written/);
    expect(onDone).not.toHaveBeenCalled();
  });
});

describe("reading an image", () => {
  it("sends the file as base64 and reviews the single account found", async () => {
    const { api } = open({ draftFromImage: vi.fn().mockResolvedValue([alice]) });
    await userEvent.click(screen.getByRole("button", { name: "Read a QR code from an image" }));
    await userEvent.upload(screen.getByLabelText(/Choose a PNG or JPEG/), pngFile(3));
    expect(api.draftFromImage).toHaveBeenCalledWith("AQEB");
    expect(await screen.findByLabelText("Account")).toHaveValue("alice");
  });

  it("asks which account to use when the image holds several", async () => {
    const { api } = open({ draftFromImage: vi.fn().mockResolvedValue([alice, bob]) });
    await userEvent.click(screen.getByRole("button", { name: "Read a QR code from an image" }));
    await userEvent.upload(screen.getByLabelText(/Choose a PNG or JPEG/), pngFile());
    await userEvent.click(await screen.findByRole("button", { name: "Example · bob" }));
    expect(await screen.findByLabelText("Account")).toHaveValue("bob");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(api.addDraft).toHaveBeenCalledWith(1, expect.objectContaining({ account: "bob" }));
  });

  it("shows why an image was refused", async () => {
    open({
      draftFromImage: vi
        .fn()
        .mockRejectedValue(new IpcCallError({ code: "invalid_input", message: "no QR code was found in the image" })),
    });
    await userEvent.click(screen.getByRole("button", { name: "Read a QR code from an image" }));
    await userEvent.upload(screen.getByLabelText(/Choose a PNG or JPEG/), pngFile());
    expect(await screen.findByRole("alert")).toHaveTextContent("no QR code was found in the image");
  });

  it("refuses a file over 10 MiB without sending it", async () => {
    const { api } = open();
    await userEvent.click(screen.getByRole("button", { name: "Read a QR code from an image" }));
    const big = new File([new Uint8Array(10 * 1024 * 1024 + 1)], "big.png", { type: "image/png" });
    await userEvent.upload(screen.getByLabelText(/Choose a PNG or JPEG/), big);
    expect(await screen.findByRole("alert")).toHaveTextContent("larger than 10 MiB");
    expect(api.draftFromImage).not.toHaveBeenCalled();
  });
});
