import { Button } from "@/components/ui/button";
import { EntryList } from "@/features/entries/EntryList";

import { useActivityReporter } from "./useActivityReporter";

export function OpenVault({ onLock }: { onLock: () => void }) {
  useActivityReporter();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex justify-end">
        <Button variant="outline" onClick={onLock}>
          Lock
        </Button>
      </div>
      <EntryList />
    </div>
  );
}
