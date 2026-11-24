import { useCallback, useEffect, useState, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { useApi, type VaultApi } from "@/ipc/api-context";
import { describeError } from "@/ipc/error-message";

import { CreateVaultScreen } from "./CreateVaultScreen";
import { UnlockScreen } from "./UnlockScreen";

type Gate =
  | { kind: "loading" }
  | { kind: "create" }
  | { kind: "unlock" }
  | { kind: "unlocked" }
  | { kind: "error"; message: string };

const STATUS_POLL_MS = 1000;

export interface SessionActions {
  lock: () => Promise<void>;
}

async function readGate(api: VaultApi, quiet: boolean): Promise<Gate | null> {
  try {
    const status = await api.vaultStatus();
    return { kind: status.unlocked ? "unlocked" : status.exists ? "unlock" : "create" };
  } catch (error) {
    return quiet ? null : { kind: "error", message: describeError(error) };
  }
}

/** Shows create / unlock until the vault is open, then `children`. Follows auto-lock. */
export function SessionGate({ children }: { children: (actions: SessionActions) => ReactNode }) {
  const api = useApi();
  const [gate, setGate] = useState<Gate>({ kind: "loading" });

  const refresh = useCallback(
    async (quiet: boolean) => {
      const next = await readGate(api, quiet);
      if (next) {
        setGate(next);
      }
    },
    [api],
  );

  useEffect(() => {
    let cancelled = false;
    void readGate(api, false).then((next) => {
      if (next && !cancelled) {
        setGate(next);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [api]);

  const unlocked = gate.kind === "unlocked";
  useEffect(() => {
    if (!unlocked) {
      return;
    }
    const timer = setInterval(() => void refresh(true), STATUS_POLL_MS);
    return () => clearInterval(timer);
  }, [unlocked, refresh]);

  const lock = useCallback(async () => {
    await api.lock();
    await refresh(false);
  }, [api, refresh]);

  switch (gate.kind) {
    case "loading":
      return <p className="p-6 text-center text-muted-foreground">Loading…</p>;
    case "create":
      return <CreateVaultScreen onCreated={() => void refresh(false)} />;
    case "unlock":
      return <UnlockScreen onUnlocked={() => void refresh(false)} />;
    case "error":
      return (
        <div className="mx-auto mt-12 flex max-w-sm flex-col items-center gap-3">
          <p role="alert" className="text-destructive">
            {gate.message}
          </p>
          <Button onClick={() => void refresh(false)}>Try again</Button>
        </div>
      );
    case "unlocked":
      return <>{children({ lock })}</>;
  }
}
