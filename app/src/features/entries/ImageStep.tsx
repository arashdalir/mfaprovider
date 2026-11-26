import { useState, type ChangeEvent } from "react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { useApi } from "@/ipc/api-context";
import type { DraftView } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

import { fileToBase64 } from "./file-to-base64";

/** Choose a PNG or JPEG with a QR code; only the core ever reads what the code says. */
export function ImageStep({
  onDrafts,
  onCancel,
}: {
  onDrafts: (drafts: DraftView[]) => void;
  onCancel: () => void;
}) {
  const api = useApi();
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function choose(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }
    setBusy(true);
    setProblem(null);
    try {
      onDrafts(await api.draftFromImage(await fileToBase64(file)));
    } catch (error) {
      setProblem(error instanceof RangeError ? error.message : describeError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section aria-label="From image" className="flex flex-col gap-4">
      <h2 className="text-lg font-semibold">From an image</h2>
      <div className="flex flex-col gap-2">
        <Label htmlFor="qr-image">Choose a PNG or JPEG image with a QR code</Label>
        <input
          id="qr-image"
          type="file"
          accept="image/png,image/jpeg"
          disabled={busy}
          onChange={(event) => void choose(event)}
        />
        <p className="text-sm text-muted-foreground">
          Images up to 10 MiB and about 5000 × 5000 pixels. A screenshot works best.
        </p>
      </div>
      {busy && <p role="status">Reading the image…</p>}
      {problem && (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      )}
      <Button type="button" variant="outline" className="self-start" onClick={onCancel}>
        Cancel
      </Button>
    </section>
  );
}
