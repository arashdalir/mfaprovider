import { useEffect, useState } from "react";

import { BackButton } from "@/components/BackButton";
import { Button } from "@/components/ui/button";
import { AddAccountScreen } from "@/features/entries/AddAccountScreen";
import { EntryForm } from "@/features/entries/EntryForm";
import { EntryList } from "@/features/entries/EntryList";
import { ImportScreen } from "@/features/import/ImportScreen";
import { SettingsScreen } from "@/features/settings/SettingsScreen";
import { idleLockSeconds, loadSettings } from "@/features/settings/settings";
import type { Theme } from "@/features/theme/theme";
import { useApi } from "@/ipc/api-context";
import { useBack } from "@/lib/use-back";
import type { EntryView } from "@/ipc/client";

import { useActivityReporter } from "./useActivityReporter";

type View =
  | { kind: "list" }
  | { kind: "add" }
  | { kind: "edit"; entry: EntryView }
  | { kind: "import" }
  | { kind: "settings" };

export function OpenVault({
  onLock,
  theme,
  onThemeChange,
}: {
  onLock: () => void;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
}) {
  const api = useApi();
  useActivityReporter();
  const [view, setView] = useState<View>({ kind: "list" });
  const backToList = () => setView({ kind: "list" });
  useBack(view.kind === "list" ? null : backToList);

  // The core forgets the chosen idle time when the app restarts; tell it again once unlocked.
  useEffect(() => {
    void api.setIdleLock(idleLockSeconds(loadSettings())).catch(() => undefined);
  }, [api]);

  function subScreen(current: Exclude<View, { kind: "list" }>) {
    switch (current.kind) {
      case "import":
        return <ImportScreen onClose={backToList} />;
      case "settings":
        return <SettingsScreen theme={theme} onThemeChange={onThemeChange} onClose={backToList} />;
      case "add":
        return <AddAccountScreen onDone={backToList} onCancel={backToList} />;
      case "edit":
        return (
          <EntryForm key={current.entry.id} entry={current.entry} onDone={backToList} onCancel={backToList} />
        );
    }
  }

  if (view.kind !== "list") {
    return (
      <div className="flex flex-col gap-2">
        <BackButton />
        {subScreen(view)}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-4">
      <div className="flex justify-between gap-2">
        <Button onClick={() => setView({ kind: "add" })}>Add account</Button>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setView({ kind: "import" })}>
            Import
          </Button>
          <Button variant="outline" onClick={() => setView({ kind: "settings" })}>
            Settings
          </Button>
          <Button variant="outline" onClick={onLock}>
            Lock
          </Button>
        </div>
      </div>
      <EntryList onEdit={(entry) => setView({ kind: "edit", entry })} />
    </div>
  );
}
