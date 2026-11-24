import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiContext, type VaultApi } from "@/ipc/api-context";
import { IpcCallError, type EntryView } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { EntryList } from "./EntryList";

function entry(overrides: Partial<EntryView> & { id: string }): EntryView {
  return {
    otp_type: "totp",
    issuer: "Issuer",
    account: "account",
    icon: null,
    tags: [],
    notes: "",
    params: {},
    created_at_ms: 0,
    updated_at_ms: 0,
    code: { state: "ready", text: "482913", period_seconds: 30, seconds_remaining: 12 },
    ...overrides,
  };
}

const github = entry({ id: "1", issuer: "GitHub", account: "alice", tags: ["work"] });
const google = entry({
  id: "2",
  issuer: "Google",
  account: "bob",
  tags: ["mail"],
  code: { state: "ready", text: "07155699", period_seconds: 30, seconds_remaining: 3 },
});
const steam = entry({
  id: "3",
  issuer: "Steam",
  account: "gamer",
  code: { state: "ready", text: "K7P2N", period_seconds: 30, seconds_remaining: 20 },
});

function show(
  entries: EntryView[],
  overrides: Partial<VaultApi> = {},
  copy = vi.fn().mockResolvedValue(undefined),
  onEdit = vi.fn(),
) {
  const api = fakeApi({ listEntries: vi.fn().mockResolvedValue(entries), ...overrides });
  render(
    <ApiContext.Provider value={api}>
      <EntryList copy={copy} onEdit={onEdit} />
    </ApiContext.Provider>,
  );
  return { api, copy, onEdit };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("EntryList", () => {
  it("shows each account with a readable code and its time left", async () => {
    show([github, google]);
    expect(await screen.findByRole("button", { name: "Copy code for GitHub · alice" })).toHaveTextContent("482 913");
    expect(screen.getByRole("button", { name: "Copy code for Google · bob" })).toHaveTextContent("0715 5699");
    const bar = screen.getByRole("progressbar", { name: "Time left for GitHub · alice" });
    expect(bar).toHaveAttribute("aria-valuenow", "12");
    expect(bar).toHaveAttribute("aria-valuemax", "30");
  });

  it("asks to edit the chosen account", async () => {
    const { onEdit } = show([github, google]);
    await userEvent.click(await screen.findByRole("button", { name: "Edit Google · bob" }));
    expect(onEdit).toHaveBeenCalledWith(google);
  });

  it("says when there are no accounts", async () => {
    show([]);
    expect(await screen.findByText("No accounts yet.")).toBeInTheDocument();
  });

  it("copies the raw code and confirms it", async () => {
    const { copy } = show([github]);
    await userEvent.click(await screen.findByRole("button", { name: /Copy code for GitHub/ }));
    expect(copy).toHaveBeenCalledWith("482913");
    expect(await screen.findByRole("status")).toHaveTextContent("Code copied");
  });

  it("reports a refused copy", async () => {
    show([github], {}, vi.fn().mockRejectedValue(new Error("denied")));
    await userEvent.click(await screen.findByRole("button", { name: /Copy code for GitHub/ }));
    expect(await screen.findByRole("status")).toHaveTextContent("Something went wrong.");
  });

  it("searches by issuer, account and tag", async () => {
    show([github, google, steam]);
    await screen.findByText(/GitHub/);
    await userEvent.type(screen.getByRole("searchbox", { name: "Search accounts" }), "bob");
    expect(screen.queryByText(/GitHub/)).not.toBeInTheDocument();
    expect(screen.getByText(/Google/)).toBeInTheDocument();
    await userEvent.clear(screen.getByRole("searchbox"));
    await userEvent.type(screen.getByRole("searchbox"), "nothing at all");
    expect(screen.getByText("No account matches.")).toBeInTheDocument();
  });

  it("filters by tag", async () => {
    show([github, google, steam]);
    await screen.findByText(/GitHub/);
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Filter by tag" }), "work");
    expect(screen.getByText(/GitHub/)).toBeInTheDocument();
    expect(screen.queryByText(/Google/)).not.toBeInTheDocument();
  });

  it("shows unsupported accounts without a code or copy button", async () => {
    const unknown = entry({
      id: "9",
      issuer: "Vendor",
      account: "x",
      otp_type: "vendor-x",
      code: { state: "unsupported" },
    });
    show([unknown]);
    expect(await screen.findByText(/Unsupported type “vendor-x”/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Copy code/ })).not.toBeInTheDocument();
  });

  it("flags a broken account", async () => {
    show([entry({ id: "8", code: { state: "failed" } })]);
    expect(await screen.findByText(/set up incorrectly/)).toBeInTheDocument();
  });

  it("offers 'Next code' for counter-based accounts", async () => {
    const hotp = entry({
      id: "7",
      otp_type: "hotp",
      code: { state: "ready", text: "755224", period_seconds: null, seconds_remaining: null },
    });
    const { api } = show([hotp]);
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    await userEvent.click(await screen.findByRole("button", { name: "Next code" }));
    expect(api.advanceCounter).toHaveBeenCalledWith("7");
  });

  it("moves accounts up and down, not past the ends", async () => {
    const { api } = show([github, google, steam]);
    await screen.findByText(/GitHub/);
    expect(screen.getByRole("button", { name: "Move GitHub · alice up" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move Steam · gamer down" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Move Google · bob up" }));
    expect(api.moveEntry).toHaveBeenCalledWith("2", 0);
    await userEvent.click(screen.getByRole("button", { name: "Move Google · bob down" }));
    expect(api.moveEntry).toHaveBeenCalledWith("2", 2);
  });

  it("does not offer moving while the list is filtered", async () => {
    show([github, google]);
    await screen.findByText(/GitHub/);
    await userEvent.type(screen.getByRole("searchbox"), "goo");
    const row = screen.getByText(/Google/).closest("li") as HTMLElement;
    expect(within(row).getByRole("button", { name: /down/ })).toBeDisabled();
    expect(within(row).getByRole("button", { name: /up/ })).toBeDisabled();
  });

  it("shows a failed move and keeps the list", async () => {
    show([github, google], {
      moveEntry: vi.fn().mockRejectedValue(new IpcCallError({ code: "storage_failed", message: null })),
    });
    await userEvent.click(await screen.findByRole("button", { name: "Move Google · bob up" }));
    expect(await screen.findByRole("status")).toHaveTextContent(/could not be read or written/);
    expect(screen.getByText(/GitHub/)).toBeInTheDocument();
  });

  it("reports a failure to load", async () => {
    show([], { listEntries: vi.fn().mockRejectedValue(new IpcCallError({ code: "locked", message: null })) });
    expect(await screen.findByRole("alert")).toHaveTextContent("The vault is locked.");
  });

  it("refreshes every second so countdowns follow the core", async () => {
    vi.useFakeTimers();
    const list = vi
      .fn()
      .mockResolvedValueOnce([github])
      .mockResolvedValue([
        { ...github, code: { state: "ready", text: "111222", period_seconds: 30, seconds_remaining: 11 } },
      ]);
    show([], { listEntries: list });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "12");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "11");
    expect(screen.getByRole("button", { name: /Copy code/ })).toHaveTextContent("111 222");
    expect(list).toHaveBeenCalledTimes(2);
  });
});
