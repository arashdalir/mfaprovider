import { useState } from "react";

import { Button } from "@/components/ui/button";
import { EntryForm } from "@/features/entries/EntryForm";
import { EntryList } from "@/features/entries/EntryList";
import type { EntryView } from "@/ipc/client";

import { useActivityReporter } from "./useActivityReporter";

type View = { kind: "list" } | { kind: "add" } | { kind: "edit"; entry: EntryView };

export function OpenVault({ onLock }: { onLock: () => void }) {
  useActivityReporter();
  const [view, setView] = useState<View>({ kind: "list" });
  const backToList = () => setView({ kind: "list" });

  if (view.kind !== "list") {
    return (
      <EntryForm
        key={view.kind === "edit" ? view.entry.id : "add"}
        entry={view.kind === "edit" ? view.entry : undefined}
        onDone={backToList}
        onCancel={backToList}
      />
    );
  }
  return (
    <div className="flex flex-col gap-4">
      <div className="flex justify-between gap-2">
        <Button onClick={() => setView({ kind: "add" })}>Add account</Button>
        <Button variant="outline" onClick={onLock}>
          Lock
        </Button>
      </div>
      <EntryList onEdit={(entry) => setView({ kind: "edit", entry })} />
    </div>
  );
}
