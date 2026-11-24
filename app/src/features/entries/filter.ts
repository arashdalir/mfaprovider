import type { EntryView } from "@/ipc/client";

/** Entries whose issuer, account or a tag contains `query` (ignoring case) and that carry `tag`. */
export function filterEntries(entries: EntryView[], query: string, tag: string | null): EntryView[] {
  const needle = query.trim().toLowerCase();
  return entries.filter((entry) => {
    const matchesText =
      needle === "" ||
      [entry.issuer, entry.account, ...entry.tags].some((text) =>
        text.toLowerCase().includes(needle),
      );
    const matchesTag =
      tag === null || entry.tags.some((candidate) => candidate.toLowerCase() === tag.toLowerCase());
    return matchesText && matchesTag;
  });
}

/** Every tag in use, once each (first spelling wins), sorted ignoring case. */
export function allTags(entries: EntryView[]): string[] {
  const seen = new Map<string, string>();
  for (const tag of entries.flatMap((entry) => entry.tags)) {
    if (!seen.has(tag.toLowerCase())) {
      seen.set(tag.toLowerCase(), tag);
    }
  }
  return [...seen.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([, tag]) => tag);
}
