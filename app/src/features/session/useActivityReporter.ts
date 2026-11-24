import { useEffect } from "react";

import { useApi } from "@/ipc/api-context";

const MIN_INTERVAL_MS = 5000;

/**
 * Tells the core the user is active (clicks, keys, touches) so the idle auto-lock waits.
 * Reports at most once per interval; polling for codes is deliberately not activity.
 */
export function useActivityReporter(): void {
  const api = useApi();
  useEffect(() => {
    let last = -Infinity;
    const report = () => {
      const now = Date.now();
      if (now - last >= MIN_INTERVAL_MS) {
        last = now;
        void api.recordActivity().catch(() => undefined);
      }
    };
    const events = ["pointerdown", "keydown"] as const;
    events.forEach((name) => window.addEventListener(name, report));
    return () => events.forEach((name) => window.removeEventListener(name, report));
  }, [api]);
}
