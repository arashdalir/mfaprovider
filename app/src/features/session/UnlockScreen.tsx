import { useEffect, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useApi } from "@/ipc/api-context";
import type { BiometricStatus } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

export function UnlockScreen({ onUnlocked }: { onUnlocked: () => void }) {
  const api = useApi();
  const [password, setPassword] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [biometrics, setBiometrics] = useState<BiometricStatus | null>(null);

  useEffect(() => {
    api.biometricStatus().then(setBiometrics, () => setBiometrics(null));
  }, [api]);

  async function unlockWithBiometrics() {
    setBusy(true);
    setProblem(null);
    try {
      await api.unlockWithBiometric();
      onUnlocked();
    } catch (error) {
      setProblem(describeError(error));
      api.biometricStatus().then(setBiometrics, () => setBiometrics(null));
    } finally {
      setBusy(false);
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setProblem(null);
    try {
      await api.unlock(password);
      setPassword("");
      onUnlocked();
    } catch (error) {
      setPassword("");
      setProblem(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="mx-auto mt-12 w-full max-w-sm">
      <CardHeader>
        <CardTitle>
          <h2>Unlock</h2>
        </CardTitle>
      </CardHeader>
      <CardContent>
        {biometrics?.enabled && !biometrics.password_due && (
          <Button
            type="button"
            variant="secondary"
            className="mb-4 w-full"
            disabled={busy}
            onClick={() => void unlockWithBiometrics()}
          >
            Unlock with fingerprint or face
          </Button>
        )}
        {biometrics?.enabled && biometrics.password_due && (
          <p className="mb-4 text-sm text-muted-foreground">
            It has been a while: please type your password once.
          </p>
        )}
        <form className="flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
          <div className="flex flex-col gap-2">
            <Label htmlFor="password">Master password</Label>
            <Input
              id="password"
              type="password"
              autoComplete="current-password"
              autoFocus
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              disabled={busy}
            />
          </div>
          {problem && (
            <p role="alert" className="text-sm text-destructive">
              {problem}
            </p>
          )}
          <Button type="submit" disabled={busy || password === ""}>
            {busy ? "Unlocking…" : "Unlock"}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
