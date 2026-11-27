import { useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { CredentialName } from "@/features/credentials/CredentialName";
import { devicePasswordName } from "@/features/credentials/credential-names";
import { useApi } from "@/ipc/api-context";
import { describeError } from "@/ipc/error-message";

import { newPasswordProblem } from "./password-rules";

export function CreateVaultScreen({ onCreated }: { onCreated: () => void }) {
  const api = useApi();
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const found = newPasswordProblem(password, confirmation);
    setProblem(found);
    if (found) {
      return;
    }
    setBusy(true);
    try {
      await api.createVault(password);
      setPassword("");
      setConfirmation("");
      onCreated();
    } catch (error) {
      setProblem(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="mx-auto mt-12 w-full max-w-sm">
      <CardHeader>
        <CardTitle>
          <h2>Create your vault</h2>
        </CardTitle>
        <CardDescription>
          Your accounts on this device are encrypted with this password. It cannot be recovered
          if you forget it.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form className="flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
          <CredentialName name={devicePasswordName()} />
          <div className="flex flex-col gap-2">
            <Label htmlFor="new-password">Device password</Label>
            <Input
              id="new-password"
              name="new-device-password"
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              disabled={busy}
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="confirm-password">Repeat device password</Label>
            <Input
              id="confirm-password"
              name="repeat-device-password"
              type="password"
              autoComplete="new-password"
              value={confirmation}
              onChange={(event) => setConfirmation(event.target.value)}
              disabled={busy}
            />
          </div>
          {problem && (
            <p role="alert" className="text-sm text-destructive">
              {problem}
            </p>
          )}
          <Button type="submit" disabled={busy}>
            {busy ? "Creating… this takes a few seconds" : "Create vault"}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
