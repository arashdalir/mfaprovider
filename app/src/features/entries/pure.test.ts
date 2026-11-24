import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EntryView } from "@/ipc/client";

import { copyWithAutoClear } from "./clipboard";
import { allTags, filterEntries } from "./filter";
import { formatCode } from "./format";

function entry(issuer: string, account: string, tags: string[] = []): EntryView {
  return {
    id: `${issuer}-${account}`,
    otp_type: "totp",
    issuer,
    account,
    icon: null,
    tags,
    notes: "",
    created_at_ms: 0,
    updated_at_ms: 0,
    code: { state: "unsupported" },
  };
}

describe("filterEntries", () => {
  const entries = [
    entry("GitHub", "alice", ["Work"]),
    entry("Google", "bob", ["work", "mail"]),
    entry("Bank", "carol"),
  ];
  const names = (list: EntryView[]) => list.map((e) => e.issuer);

  it("matches issuer, account and tags ignoring case", () => {
    expect(names(filterEntries(entries, "GIT", null))).toEqual(["GitHub"]);
    expect(names(filterEntries(entries, "bob", null))).toEqual(["Google"]);
    expect(names(filterEntries(entries, "mai", null))).toEqual(["Google"]);
  });

  it("returns everything for a blank query", () => {
    expect(filterEntries(entries, "  ", null)).toHaveLength(3);
  });

  it("filters by exact tag ignoring case, combined with the query", () => {
    expect(names(filterEntries(entries, "", "WORK"))).toEqual(["GitHub", "Google"]);
    expect(names(filterEntries(entries, "goo", "work"))).toEqual(["Google"]);
    expect(filterEntries(entries, "", "missing")).toHaveLength(0);
  });

  it("does not match on notes", () => {
    const noted = { ...entry("A", "a"), notes: "recovery phrase" };
    expect(filterEntries([noted], "recovery", null)).toHaveLength(0);
  });
});

describe("allTags", () => {
  it("lists each tag once, first spelling, sorted ignoring case", () => {
    const entries = [entry("A", "a", ["work", "Mail"]), entry("B", "b", ["MAIL", "Zed"])];
    expect(allTags(entries)).toEqual(["Mail", "work", "Zed"]);
  });
});

describe("formatCode", () => {
  it("splits even-length numeric codes in the middle", () => {
    expect(formatCode("482913")).toBe("482 913");
    expect(formatCode("48291357")).toBe("4829 1357");
  });

  it("leaves other codes alone", () => {
    expect(formatCode("K7P2N")).toBe("K7P2N");
    expect(formatCode("1234567")).toBe("1234567");
  });
});

describe("copyWithAutoClear", () => {
  const writeText = vi.fn();
  const readText = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    writeText.mockReset().mockResolvedValue(undefined);
    readText.mockReset();
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText, readText },
      configurable: true,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("copies right away and clears after the delay", async () => {
    readText.mockResolvedValue("123456");
    await copyWithAutoClear("123456", 1000);
    expect(writeText).toHaveBeenLastCalledWith("123456");
    await vi.advanceTimersByTimeAsync(1000);
    expect(writeText).toHaveBeenLastCalledWith("");
  });

  it("leaves the clipboard alone if the user copied something else meanwhile", async () => {
    readText.mockResolvedValue("something the user copied");
    await copyWithAutoClear("123456", 1000);
    await vi.advanceTimersByTimeAsync(1000);
    expect(writeText).toHaveBeenCalledTimes(1);
  });

  it("clears anyway when the clipboard cannot be read", async () => {
    readText.mockRejectedValue(new Error("denied"));
    await copyWithAutoClear("123456", 1000);
    await vi.advanceTimersByTimeAsync(1000);
    expect(writeText).toHaveBeenLastCalledWith("");
  });

  it("a newer copy restarts the clock instead of being cleared early", async () => {
    readText.mockImplementation(() => Promise.resolve(writeText.mock.calls.at(-1)?.[0]));
    await copyWithAutoClear("111111", 1000);
    await vi.advanceTimersByTimeAsync(600);
    await copyWithAutoClear("222222", 1000);
    await vi.advanceTimersByTimeAsync(600);
    expect(writeText).toHaveBeenLastCalledWith("222222");
    await vi.advanceTimersByTimeAsync(400);
    expect(writeText).toHaveBeenLastCalledWith("");
  });

  it("reports a refused copy to the caller", async () => {
    writeText.mockRejectedValue(new Error("no permission"));
    await expect(copyWithAutoClear("123456", 1000)).rejects.toThrow("no permission");
  });
});
