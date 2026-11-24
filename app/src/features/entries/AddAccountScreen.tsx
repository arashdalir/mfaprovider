import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApi } from "@/ipc/api-context";
import type { DraftView } from "@/ipc/client";

import { EntryForm } from "./EntryForm";
import { ImageStep } from "./ImageStep";
import { LinkStep } from "./LinkStep";

type Step =
  | { kind: "choose" }
  | { kind: "manual" }
  | { kind: "link" }
  | { kind: "image" }
  | { kind: "review"; drafts: DraftView[]; chosen: DraftView | null };

/** Add an account by typing it in, pasting an otpauth:// link, or reading a QR image. */
export function AddAccountScreen({ onDone, onCancel }: { onDone: () => void; onCancel: () => void }) {
  const api = useApi();
  const [step, setStep] = useState<Step>({ kind: "choose" });
  const choose = () => setStep({ kind: "choose" });

  function review(drafts: DraftView[]) {
    setStep({ kind: "review", drafts, chosen: drafts.length === 1 ? drafts[0] : null });
  }

  function abandonDrafts() {
    void api.discardDrafts().catch(() => undefined);
    choose();
  }

  switch (step.kind) {
    case "manual":
      return <EntryForm onDone={onDone} onCancel={choose} />;
    case "link":
      return <LinkStep onDrafts={review} onCancel={choose} />;
    case "image":
      return <ImageStep onDrafts={review} onCancel={choose} />;
    case "review":
      return step.chosen ? (
        <EntryForm draft={step.chosen} onDone={onDone} onCancel={abandonDrafts} />
      ) : (
        <section aria-label="Choose account" className="flex flex-col gap-3">
          <h2 className="text-lg font-semibold">Several accounts were found</h2>
          <ul className="flex flex-col gap-2">
            {step.drafts.map((draft) => (
              <li key={draft.draft_id}>
                <Button
                  variant="outline"
                  className="w-full justify-start"
                  onClick={() => setStep({ ...step, chosen: draft })}
                >
                  {draft.issuer} · {draft.account}
                </Button>
              </li>
            ))}
          </ul>
          <Button variant="outline" className="self-start" onClick={abandonDrafts}>
            Cancel
          </Button>
        </section>
      );
    case "choose":
      return (
        <section aria-label="Add account" className="flex flex-col gap-3">
          <h2 className="text-lg font-semibold">Add account</h2>
          <Button onClick={() => setStep({ kind: "manual" })}>Enter details manually</Button>
          <Button onClick={() => setStep({ kind: "link" })}>Paste an otpauth:// link</Button>
          <Button onClick={() => setStep({ kind: "image" })}>Read a QR code from an image</Button>
          <Button variant="outline" className="self-start" onClick={onCancel}>
            Cancel
          </Button>
        </section>
      );
  }
}
