import { useMemo, useState } from "react";

import { Input } from "@/components/ui/input";
import { useApi } from "@/ipc/api-context";
import type { EntryView } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

import { loadSettings } from "@/features/settings/settings";

import { copyWithAutoClear } from "./clipboard";
import { EntryRow } from "./EntryRow";
import { allTags, filterEntries } from "./filter";
import { useEntries } from "./useEntries";

function copyWithConfiguredDelay(text: string): Promise<void> {
  return copyWithAutoClear(text, loadSettings().clipboardClearSeconds * 1000);
}

export function EntryList({
  onEdit,
  copy = copyWithConfiguredDelay,
}: {
  onEdit: (entry: EntryView) => void;
  copy?: (text: string) => Promise<void>;
}) {
  const api = useApi();
  const { entries, error, reload } = useEntries();
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const tags = useMemo(() => allTags(entries ?? []), [entries]);
  const shown = useMemo(() => filterEntries(entries ?? [], query, tag), [entries, query, tag]);
  const hideCodes = loadSettings().hideCodes;
  const filtering = query.trim() !== "" || tag !== null;

  async function run(action: () => Promise<unknown>, success?: string) {
    try {
      await action();
      setNotice(success ?? null);
      await reload();
    } catch (failure) {
      setNotice(describeError(failure));
    }
  }

  if (entries === null) {
    return error ? (
      <p role="alert" className="text-destructive">
        {error}
      </p>
    ) : (
      <p className="text-muted-foreground">Loading accounts…</p>
    );
  }

  return (
    <section aria-label="Accounts" className="flex flex-col gap-3">
      <div className="flex gap-2">
        <Input
          type="search"
          aria-label="Search accounts"
          placeholder="Search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <select
          aria-label="Filter by tag"
          className="rounded-md border bg-background px-2"
          value={tag ?? ""}
          onChange={(event) => setTag(event.target.value === "" ? null : event.target.value)}
        >
          <option value="">All tags</option>
          {tags.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <p role="status" className="min-h-5 text-sm text-muted-foreground">
        {notice}
      </p>
      {entries.length === 0 ? (
        <p className="text-muted-foreground">No accounts yet.</p>
      ) : shown.length === 0 ? (
        <p className="text-muted-foreground">No account matches.</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {shown.map((entry) => {
            const position = entries.findIndex((candidate) => candidate.id === entry.id);
            const canMove = !filtering;
            return (
              <EntryRow
                key={entry.id}
                entry={entry}
                hideCodes={hideCodes}
                onCopy={(code) => void run(() => copy(code), "Code copied. It will be cleared shortly.")}
                onNextCode={() => void run(() => api.advanceCounter(entry.id))}
                onEdit={() => onEdit(entry)}
                onMoveUp={
                  canMove && position > 0
                    ? () => void run(() => api.moveEntry(entry.id, position - 1))
                    : undefined
                }
                onMoveDown={
                  canMove && position < entries.length - 1
                    ? () => void run(() => api.moveEntry(entry.id, position + 1))
                    : undefined
                }
              />
            );
          })}
        </ul>
      )}
    </section>
  );
}
