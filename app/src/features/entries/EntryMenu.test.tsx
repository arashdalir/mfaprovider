import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiContext, type VaultApi } from "@/ipc/api-context";
import type { EntryView } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { EntryList } from "./EntryList";

const github: EntryView = {
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
const counterBased: EntryView = {
  ...github,
  otp_type: "hotp",
  code: { state: "ready", text: "755224", period_seconds: null, seconds_remaining: null },
};
const unsupported: EntryView = { ...github, otp_type: "future", code: { state: "unsupported" } };

afterEach(() => {
  window.localStorage.clear();
});

async function openMenu(entry: EntryView = github, overrides: Partial<VaultApi> = {}) {
  const api = fakeApi({ listEntries: vi.fn().mockResolvedValue([entry]), ...overrides });
  const copy = vi.fn().mockResolvedValue(undefined);
  const onEdit = vi.fn();
  render(
    <ApiContext.Provider value={api}>
      <EntryList copy={copy} onEdit={onEdit} />
    </ApiContext.Provider>,
  );
  const browserMenuAllowed = fireEvent.contextMenu(await screen.findByText("GitHub · alice"));
  await screen.findByRole("menu");
  return { api, copy, onEdit, browserMenuAllowed };
}

const items = () => screen.getAllByRole("menuitem").map((item) => item.textContent);
const choose = (name: string) => userEvent.click(screen.getByRole("menuitem", { name }));

describe("an account's context menu", () => {
  it("replaces the browser's menu with the account's actions", async () => {
    const { browserMenuAllowed } = await openMenu();
    expect(browserMenuAllowed).toBe(false);
    expect(items()).toEqual(["Copy code", "Show code", "Edit…", "Show secret key…", "Delete…"]);
  });

  it("copies the code", async () => {
    const { copy } = await openMenu();
    await choose("Copy code");
    expect(copy).toHaveBeenCalledWith("482913");
  });

  it("shows a hidden code and offers to hide it again", async () => {
    await openMenu();
    await choose("Show code");
    const code = screen.getByRole("button", { name: "Copy code for GitHub · alice" });
    expect(code).toHaveTextContent("482 913");
    fireEvent.contextMenu(screen.getByText("GitHub · alice"));
    await choose("Hide code");
    expect(code).not.toHaveTextContent("482 913");
  });

  it("offers no show or hide when codes are always shown", async () => {
    window.localStorage.setItem("mfa-settings", JSON.stringify({ hideCodes: false }));
    await openMenu();
    expect(items()).toEqual(["Copy code", "Edit…", "Show secret key…", "Delete…"]);
  });

  it("moves a counter-based account to its next code", async () => {
    const { api } = await openMenu(counterBased, { advanceCounter: vi.fn().mockResolvedValue(undefined) });
    await choose("Next code");
    expect(api.advanceCounter).toHaveBeenCalledWith("1");
  });

  it("offers nothing about codes for a type it cannot compute", async () => {
    await openMenu(unsupported);
    expect(items()).toEqual(["Edit…", "Show secret key…", "Delete…"]);
  });

  it.each([
    ["Edit…", undefined],
    ["Show secret key…", "reveal"],
    ["Delete…", "delete"],
  ])("opens the account for “%s”, deleting nothing by itself", async (name, startWith) => {
    const { api, onEdit } = await openMenu();
    await choose(name!);
    expect(onEdit).toHaveBeenCalledWith(github, startWith);
    expect(api.deleteEntry).not.toHaveBeenCalled();
  });
});
