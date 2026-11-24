import { ChevronDown, ChevronUp } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { EntryView } from "@/ipc/client";

import { formatCode } from "./format";

const LOW_SECONDS = 5;

export interface EntryRowProps {
  entry: EntryView;
  onCopy: (code: string) => void;
  onNextCode: () => void;
  /** Absent while the list is filtered, when moving would be ambiguous. */
  onMoveUp?: () => void;
  onMoveDown?: () => void;
}

export function EntryRow({ entry, onCopy, onNextCode, onMoveUp, onMoveDown }: EntryRowProps) {
  const title = `${entry.issuer} · ${entry.account}`;
  return (
    <li className="flex items-center gap-2 rounded-lg border p-3">
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm text-muted-foreground">{title}</p>
        <CodeArea entry={entry} title={title} onCopy={onCopy} onNextCode={onNextCode} />
      </div>
      <div className="flex flex-col">
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={`Move ${title} up`}
          disabled={!onMoveUp}
          onClick={onMoveUp}
        >
          <ChevronUp />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={`Move ${title} down`}
          disabled={!onMoveDown}
          onClick={onMoveDown}
        >
          <ChevronDown />
        </Button>
      </div>
    </li>
  );
}

function CodeArea({
  entry,
  title,
  onCopy,
  onNextCode,
}: Pick<EntryRowProps, "entry" | "onCopy" | "onNextCode"> & { title: string }) {
  const { code } = entry;
  if (code.state === "unsupported") {
    return (
      <p className="text-sm">
        Unsupported type “{entry.otp_type}” — kept as it is, no code shown.
      </p>
    );
  }
  if (code.state === "failed") {
    return <p className="text-sm text-destructive">This account is set up incorrectly.</p>;
  }
  const remaining = code.seconds_remaining;
  const period = code.period_seconds;
  return (
    <div className="flex items-center gap-3">
      <Button
        variant="ghost"
        className="font-mono text-2xl tracking-wider"
        aria-label={`Copy code for ${title}`}
        onClick={() => onCopy(code.text)}
      >
        {formatCode(code.text)}
      </Button>
      {remaining !== null && period !== null ? (
        <div
          role="progressbar"
          aria-label={`Time left for ${title}`}
          aria-valuemin={0}
          aria-valuemax={period}
          aria-valuenow={remaining}
          aria-valuetext={`${remaining} seconds`}
          className="flex flex-1 items-center gap-2"
        >
          <div className="h-1.5 flex-1 overflow-hidden rounded bg-muted">
            <div
              className={remaining <= LOW_SECONDS ? "h-full bg-destructive" : "h-full bg-primary"}
              style={{ width: `${(remaining / period) * 100}%` }}
            />
          </div>
          <span className="w-8 text-right text-sm tabular-nums">{remaining}s</span>
        </div>
      ) : (
        <Button variant="outline" size="sm" onClick={onNextCode}>
          Next code
        </Button>
      )}
    </div>
  );
}
