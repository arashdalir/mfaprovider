import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { useApi } from "@/ipc/api-context";
import type { ImportDecision, ImportItemView, ImportPreview, ImportResult, ImportStatus } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

const STATUS_TEXT: Record<ImportStatus, string> = {
  new: "New",
  duplicate_in_vault: "Already in the vault",
  duplicate_in_file: "Repeated in this file",
  conflict: "Same name, different key",
  already_under_other_name: "Already in the vault under another name",
};

function itemTitle(item: ImportItemView): string {
  return item.issuer === "" ? item.account : `${item.issuer} · ${item.account}`;
}

/** Shows what importing would do, lets the user change it per account, and applies it. */
export function ImportPreviewStep({
  preview,
  onApplied,
  onCancel,
}: {
  preview: ImportPreview;
  onApplied: (result: ImportResult) => void;
  onCancel: () => void;
}) {
  const api = useApi();
  const suggested = useMemo(
    () => Object.fromEntries(preview.items.map((item) => [item.item_id, item.default_decision])),
    [preview],
  );
  const [decisions, setDecisions] = useState<Record<number, ImportDecision>>(suggested);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const chosen = Object.values(decisions);
  const adding = chosen.filter((decision) => decision === "add").length;
  const replacing = chosen.filter((decision) => decision === "replace").length;

  async function apply() {
    setBusy(true);
    try {
      onApplied(
        await api.importApply(
          preview.items.map((item) => ({ item_id: item.item_id, decision: decisions[item.item_id] })),
        ),
      );
    } catch (error) {
      setProblem(describeError(error));
      setBusy(false);
    }
  }

  const set = (id: number, decision: ImportDecision) =>
    setDecisions((current) => ({ ...current, [id]: decision }));

  return (
    <section aria-label="Import preview" className="flex flex-col gap-4">
      <h2 className="text-lg font-semibold">Review the import</h2>
      <p>
        {preview.items.length} {preview.items.length === 1 ? "account" : "accounts"} found
        {preview.skipped.length > 0 && `, ${preview.skipped.length} lines could not be read`}.
      </p>

      {preview.skipped.length > 0 && (
        <details>
          <summary className="cursor-pointer">Lines that could not be read</summary>
          <ul className="mt-2 list-disc pl-5 text-sm">
            {preview.skipped.map((line) => (
              <li key={line.position}>
                Line {line.position}: {line.reason}
              </li>
            ))}
          </ul>
        </details>
      )}

      {preview.items.length > 0 && (
        <>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                setDecisions(Object.fromEntries(preview.items.map((item) => [item.item_id, "skip"])))
              }
            >
              Skip all
            </Button>
            <Button variant="outline" size="sm" onClick={() => setDecisions(suggested)}>
              Suggested choices
            </Button>
          </div>
          <ul className="flex flex-col gap-2">
            {preview.items.map((item) => (
              <li key={item.item_id} className="flex flex-col gap-1 rounded-lg border p-3">
                <p className="font-medium">{itemTitle(item)}</p>
                <p className="text-sm text-muted-foreground">
                  {STATUS_TEXT[item.status]}
                  {item.matches !== null && ` (${item.matches})`}
                </p>
                <Choice item={item} value={decisions[item.item_id]} onChange={(d) => set(item.item_id, d)} />
              </li>
            ))}
          </ul>
        </>
      )}

      <p role="status">
        Will add {adding}, replace {replacing}, skip {preview.items.length - adding - replacing}.
      </p>
      {problem && (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      )}
      <div className="flex gap-2">
        <Button disabled={busy || adding + replacing === 0} onClick={() => void apply()}>
          Import {adding + replacing} {adding + replacing === 1 ? "account" : "accounts"}
        </Button>
        <Button variant="outline" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
      </div>
    </section>
  );
}

function Choice({
  item,
  value,
  onChange,
}: {
  item: ImportItemView;
  value: ImportDecision;
  onChange: (decision: ImportDecision) => void;
}) {
  const title = itemTitle(item);
  if (item.status === "new") {
    return (
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={value === "add"}
          onChange={(event) => onChange(event.target.checked ? "add" : "skip")}
        />
        Import {title}
      </label>
    );
  }
  return (
    <select
      aria-label={`Action for ${title}`}
      className="rounded-md border bg-background px-2 py-1 text-sm"
      value={value}
      onChange={(event) => onChange(event.target.value as ImportDecision)}
    >
      <option value="skip">Skip</option>
      <option value="add">Keep both (add as new)</option>
      {item.can_replace && <option value="replace">Replace the existing one</option>}
    </select>
  );
}
