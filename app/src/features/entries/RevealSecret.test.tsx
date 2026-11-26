import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiContext } from "@/ipc/api-context";
import { IpcCallError } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { REVEAL_VISIBLE_MS, RevealSecret } from "./RevealSecret";

function open(revealSecret = vi.fn().mockResolvedValue({ secret_base32: "JBSWY3DPEHPK3PXP", qr_svg: null })) {
  const api = fakeApi({ revealSecret });
  const view = render(
    <ApiContext.Provider value={api}>
      <RevealSecret entryId="abc" />
    </ApiContext.Provider>,
  );
  return { api, view };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("RevealSecret", () => {
  it("shows nothing until asked, and nothing is fetched yet", () => {
    const { api } = open();
    expect(screen.getByRole("button", { name: "Show secret key…" })).toBeInTheDocument();
    expect(api.revealSecret).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("Secret key")).not.toBeInTheDocument();
  });

  it("reveals the secret after the password is entered", async () => {
    const { api } = open();
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    await userEvent.type(screen.getByLabelText("Master password"), "my password");
    await userEvent.click(screen.getByRole("button", { name: "Reveal" }));
    expect(api.revealSecret).toHaveBeenCalledWith("abc", "my password");
    expect(await screen.findByLabelText("Secret key")).toHaveTextContent("JBSWY3DPEHPK3PXP");
  });

  it("rejects a wrong password without showing anything", async () => {
    open(vi.fn().mockRejectedValue(new IpcCallError({ code: "wrong_password", message: null })));
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    await userEvent.type(screen.getByLabelText("Master password"), "nope");
    await userEvent.click(screen.getByRole("button", { name: "Reveal" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Wrong password.");
    expect(screen.getByLabelText("Master password")).toHaveValue("");
    expect(screen.queryByLabelText("Secret key")).not.toBeInTheDocument();
  });

  it("shows the account's QR code next to the key, and hides it with the key", async () => {
    open(
      vi.fn().mockResolvedValue({
        secret_base32: "JBSWY3DPEHPK3PXP",
        qr_svg: "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    await userEvent.type(screen.getByLabelText("Master password"), "pw{enter}");
    const picture = await screen.findByRole("img", { name: "QR code of this account" });
    expect(picture.getAttribute("src")).toMatch(/^data:image\/svg\+xml;charset=utf-8,%3Csvg/);
    await userEvent.click(screen.getByRole("button", { name: "Hide" }));
    expect(screen.queryByRole("img", { name: "QR code of this account" })).not.toBeInTheDocument();
  });

  it("shows no picture for a type that has no QR form", async () => {
    open();
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    await userEvent.type(screen.getByLabelText("Master password"), "pw{enter}");
    await screen.findByLabelText("Secret key");
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("hides the secret on request", async () => {
    open();
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    await userEvent.type(screen.getByLabelText("Master password"), "pw{enter}");
    await userEvent.click(await screen.findByRole("button", { name: "Hide" }));
    expect(screen.queryByText("JBSWY3DPEHPK3PXP")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show secret key…" })).toBeInTheDocument();
  });

  it("hides the secret by itself after thirty seconds", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    open();
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    await userEvent.type(screen.getByLabelText("Master password"), "pw{enter}");
    expect(await screen.findByLabelText("Secret key")).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(REVEAL_VISIBLE_MS);
    });
    expect(screen.queryByLabelText("Secret key")).not.toBeInTheDocument();
  });

  it("pressing Enter in the password box does not submit an enclosing form", async () => {
    const submitted = vi.fn((event: React.FormEvent) => event.preventDefault());
    render(
      <ApiContext.Provider value={fakeApi()}>
        <form onSubmit={submitted}>
          <RevealSecret entryId="abc" />
        </form>
      </ApiContext.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Show secret key…" }));
    await userEvent.type(screen.getByLabelText("Master password"), "pw{enter}");
    expect(submitted).not.toHaveBeenCalled();
  });
});
