import { useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { useApi } from "@/ipc/api-context";
import type { DraftView } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

/** Paste an otpauth:// link; the core reads it and keeps the secret. */
export function LinkStep({
  onDrafts,
  onCancel,
}: {
  onDrafts: (drafts: DraftView[]) => void;
  onCancel: () => void;
}) {
  const api = useApi();
  const [text, setText] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const drafts = await api.draftFromLink(text);
      setText("");
      onDrafts(drafts);
    } catch (error) {
      setProblem(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="flex flex-col gap-4" onSubmit={(event) => void submit(event)} aria-label="Paste link">
      <h2 className="text-lg font-semibold">Paste a link</h2>
      <div className="flex flex-col gap-2">
        <Label htmlFor="otpauth-link">The otpauth:// link</Label>
        <textarea
          id="otpauth-link"
          className="min-h-24 rounded-md border bg-background px-3 py-2 font-mono text-sm"
          autoComplete="off"
          spellCheck={false}
          value={text}
          onChange={(event) => setText(event.target.value)}
          disabled={busy}
        />
        <p className="text-sm text-muted-foreground">
          Also accepts a Google Authenticator export link (otpauth-migration://); every account in it is
          offered for review.
        </p>
      </div>
      {problem && (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      )}
      <div className="flex gap-2">
        <Button type="submit" disabled={busy || text.trim() === ""}>
          Read link
        </Button>
        <Button type="button" variant="outline" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
