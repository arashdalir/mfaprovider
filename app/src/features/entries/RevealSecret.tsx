import { useEffect, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { CredentialName } from "@/features/credentials/CredentialName";
import { devicePasswordName } from "@/features/credentials/credential-names";
import { useApi } from "@/ipc/api-context";
import { describeError } from "@/ipc/error-message";

export const REVEAL_VISIBLE_MS = 30_000;

type Phase =
  | { kind: "hidden" }
  | { kind: "asking" }
  | { kind: "shown"; secret: string; qrSvg: string | null };

const svgSource = (svg: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;

/** Shows an account's secret key, but only after the device password is entered again. */
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

  async function reveal(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const { secret_base32, qr_svg } = await api.revealSecret(entryId, password);
      setPassword("");
      setProblem(null);
      setPhase({ kind: "shown", secret: secret_base32, qrSvg: qr_svg });
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
        {phase.qrSvg !== null && (
          <>
            <img
              alt="QR code of this account"
              src={svgSource(phase.qrSvg)}
              className="size-56 self-center rounded-md bg-white"
            />
            <p className="text-sm text-muted-foreground">
              Scan it with the other device's authenticator. Anyone who scans it gets this account.
            </p>
          </>
        )}
        <Button type="button" variant="outline" className="self-start" onClick={() => setPhase({ kind: "hidden" })}>
          Hide
        </Button>
      </div>
    );
  }
  if (phase.kind === "asking") {
    return (
      <form className="flex flex-col gap-2 rounded-md border p-3" onSubmit={(event) => void reveal(event)}>
        <CredentialName name={devicePasswordName()} />
        <Label htmlFor="reveal-password">Device password</Label>
        <Input
          id="reveal-password"
          name="device-password"
          type="password"
          autoComplete="current-password"
          autoFocus
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        {problem && (
          <p role="alert" className="text-sm text-destructive">
            {problem}
          </p>
        )}
        <div className="flex gap-2">
          <Button type="submit" disabled={busy || password === ""}>
            Reveal
          </Button>
          <Button type="button" variant="outline" onClick={() => setPhase({ kind: "hidden" })}>
            Cancel
          </Button>
        </div>
      </form>
    );
  }
  return (
    <Button type="button" variant="outline" className="self-start" onClick={() => setPhase({ kind: "asking" })}>
      Show secret key…
    </Button>
  );
}
