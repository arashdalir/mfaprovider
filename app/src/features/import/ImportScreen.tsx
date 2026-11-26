import { useEffect, useState, type ChangeEvent, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { fileToBase64 } from "@/features/entries/file-to-base64";
import { useApi } from "@/ipc/api-context";
import type { ImportFormatView, ImportPreview, ImportResult } from "@/ipc/client";
import { describeError, isPasswordRequired } from "@/ipc/error-message";
import { useBack } from "@/lib/use-back";

import { ImportPreviewStep } from "./ImportPreviewStep";

const MAX_IMPORT_FILE_BYTES = 5 * 1024 * 1024;

type Step =
  | { kind: "pick" }
  | { kind: "preview"; preview: ImportPreview }
  | { kind: "done"; result: ImportResult };

/** Import accounts from a file: choose it, review what would happen, then apply. */
export function ImportScreen({ onClose }: { onClose: () => void }) {
  const api = useApi();
  const [step, setStep] = useState<Step>({ kind: "pick" });
  const [formats, setFormats] = useState<ImportFormatView[]>([]);
  const [format, setFormat] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [locked, setLocked] = useState<File | null>(null);
  const [password, setPassword] = useState("");

  useEffect(() => {
    let cancelled = false;
    api
      .listImportFormats()
      .then((list) => {
        if (!cancelled) {
          setFormats(list);
          setFormat(list[0]?.id ?? "");
        }
      })
      .catch((error: unknown) => !cancelled && setProblem(describeError(error)));
    return () => {
      cancelled = true;
    };
  }, [api]);

  async function choose(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }
    setLocked(null);
    setPassword("");
    await open(file, null);
  }

  async function open(file: File, archivePassword: string | null) {
    setBusy(true);
    setProblem(null);
    try {
      const content = await fileToBase64(file, MAX_IMPORT_FILE_BYTES);
      setStep({ kind: "preview", preview: await api.importPreview(format, content, archivePassword) });
      setLocked(null);
      setPassword("");
    } catch (error) {
      if (isPasswordRequired(error)) {
        setLocked(file);
      }
      setProblem(error instanceof RangeError ? error.message : describeError(error));
    } finally {
      setBusy(false);
    }
  }

  function submitPassword(event: FormEvent) {
    event.preventDefault();
    if (locked !== null) {
      void open(locked, password);
    }
  }

  function cancel() {
    void api.importDiscard().catch(() => undefined);
    onClose();
  }

  function backToPick() {
    void api.importDiscard().catch(() => undefined);
    setStep({ kind: "pick" });
  }

  useBack(step.kind === "preview" ? backToPick : null);

  if (step.kind === "preview") {
    return (
      <ImportPreviewStep
        preview={step.preview}
        onApplied={(result) => setStep({ kind: "done", result })}
        onCancel={cancel}
      />
    );
  }
  if (step.kind === "done") {
    const { added, replaced, skipped } = step.result;
    return (
      <section aria-label="Import finished" className="flex flex-col gap-4">
        <h2 className="text-lg font-semibold">Import finished</h2>
        <p role="status">
          Added {added}, replaced {replaced}, skipped {skipped}.
        </p>
        <Button className="self-start" onClick={onClose}>
          Done
        </Button>
      </section>
    );
  }
  return (
    <section aria-label="Import" className="flex flex-col gap-4">
      <h2 className="text-lg font-semibold">Import accounts</h2>
      <div className="flex flex-col gap-2">
        <Label htmlFor="import-format">File format</Label>
        <select
          id="import-format"
          className="rounded-md border bg-background px-2 py-2"
          value={format}
          onChange={(event) => setFormat(event.target.value)}
        >
          {formats.map((candidate) => (
            <option key={candidate.id} value={candidate.id}>
              {candidate.label}
            </option>
          ))}
        </select>
      </div>
      <div className="flex flex-col gap-2">
        <Label htmlFor="import-file">Choose the export file</Label>
        <input
          id="import-file"
          type="file"
          accept=".txt,.json,.zip,text/plain,application/json,application/zip"
          disabled={busy || format === ""}
          onChange={(event) => void choose(event)}
        />
        <p className="text-sm text-muted-foreground">
          Nothing is changed until you have reviewed the list. The file contains secret keys: delete
          it once you are done.
        </p>
      </div>
      {locked !== null && (
        <form className="flex flex-col gap-2" onSubmit={submitPassword}>
          <Label htmlFor="archive-password">Password of the ZIP file</Label>
          <Input
            id="archive-password"
            type="password"
            autoComplete="off"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
          <Button type="submit" className="self-start" disabled={busy || password === ""}>
            Open file
          </Button>
        </form>
      )}
      {busy && <p role="status">Reading the file…</p>}
      {problem && (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      )}
    </section>
  );
}
