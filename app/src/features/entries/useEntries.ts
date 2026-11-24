import { useCallback, useEffect, useRef, useState } from "react";

import { useApi } from "@/ipc/api-context";
import type { EntryView } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

const REFRESH_MS = 1000;

/** The entries with their current codes, refreshed every second so countdowns stay true. */
export function useEntries(): {
  entries: EntryView[] | null;
  error: string | null;
  reload: () => Promise<void>;
} {
  const api = useApi();
  const [entries, setEntries] = useState<EntryView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const active = useRef(true);

  const reload = useCallback(async () => {
    try {
      const list = await api.listEntries();
      if (active.current) {
        setEntries(list);
        setError(null);
      }
    } catch (failure) {
      if (active.current) {
        setError(describeError(failure));
      }
    }
  }, [api]);

  useEffect(() => {
    active.current = true;
    const first = setTimeout(() => void reload(), 0);
    const timer = setInterval(() => void reload(), REFRESH_MS);
    return () => {
      active.current = false;
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [reload]);

  return { entries, error, reload };
}
