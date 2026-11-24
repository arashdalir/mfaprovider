import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useApi } from "@/ipc/api-context";
import { describeError } from "@/ipc/error-message";

export const REVEAL_VISIBLE_MS = 30_000;

type Phase = { kind: "hidden" } | { kind: "asking" } | { kind: "shown"; secret: string };

/** Shows an account's secret key, but only after the master password is entered again. */
export function RevealSecret({ entryId }: { entryId: string }) {
  const api = useApi();
  const [phase, setPhase] = useState<Phase>({ kind: "hidden" });
  const [password, setPassword] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const shown = phase.kind === "shown";
  useEffect(() => {
    if (!shown) {
      return;
    }
    const timer = setTimeout(() => setPhase({ kind: "hidden" }), REVEAL_VISIBLE_MS);
    return () => clearTimeout(timer);
  }, [shown]);

  async function reveal(event: { preventDefault: () => void; stopPropagation: () => void }) {
    event.preventDefault();
    event.stopPropagation();
    setBusy(true);
    try {
      const { secret_base32 } = await api.revealSecret(entryId, password);
      setPassword("");
      setProblem(null);
      setPhase({ kind: "shown", secret: secret_base32 });
    } catch (error) {
      setPassword("");
      setProblem(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  if (phase.kind === "shown") {
    return (
      <div className="flex flex-col gap-2 rounded-md border p-3">
        <p className="text-sm">Secret key (hides itself in 30 seconds):</p>
        <code aria-label="Secret key" className="break-all font-mono">
          {phase.secret}
        </code>
        <Button type="button" variant="outline" className="self-start" onClick={() => setPhase({ kind: "hidden" })}>
          Hide
        </Button>
      </div>
    );
  }
  if (phase.kind === "asking") {
    return (
      <div className="flex flex-col gap-2 rounded-md border p-3">
        <Label htmlFor="reveal-password">Master password</Label>
        <Input
          id="reveal-password"
          type="password"
          autoComplete="current-password"
          autoFocus
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              void reveal(event);
            }
          }}
        />
        {problem && (
          <p role="alert" className="text-sm text-destructive">
            {problem}
          </p>
        )}
        <div className="flex gap-2">
          <Button type="button" disabled={busy || password === ""} onClick={(event) => void reveal(event)}>
            Reveal
          </Button>
          <Button type="button" variant="outline" onClick={() => setPhase({ kind: "hidden" })}>
            Cancel
          </Button>
        </div>
      </div>
    );
  }
  return (
    <Button type="button" variant="outline" className="self-start" onClick={() => setPhase({ kind: "asking" })}>
      Show secret key…
    </Button>
  );
}
