import { ChevronDown, ChevronUp, Copy, Eye, EyeOff, Pencil } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import type { EntryView } from "@/ipc/client";

import { formatCode } from "./format";

const LOW_SECONDS = 5;
export const REVEAL_SECONDS = 30;
const MASKED_CODE = "••• •••";

/** A revealed code hides itself again after `REVEAL_SECONDS`. */
function useReveal() {
  const [revealed, setRevealed] = useState(false);
  useEffect(() => {
    if (!revealed) return;
    const timer = window.setTimeout(() => setRevealed(false), REVEAL_SECONDS * 1000);
    return () => window.clearTimeout(timer);
  }, [revealed]);
  return { revealed, show: () => setRevealed(true), hide: () => setRevealed(false) };
}

export interface EntryRowProps {
  entry: EntryView;
  /** When true the code stays masked until the user presses Show. */
  hideCodes: boolean;
  onCopy: (code: string) => void;
  onNextCode: () => void;
  onEdit: () => void;
  /** Absent while the list is filtered, when moving would be ambiguous. */
  onMoveUp?: () => void;
  onMoveDown?: () => void;
}

export function EntryRow({
  entry,
  hideCodes,
  onCopy,
  onNextCode,
  onEdit,
  onMoveUp,
  onMoveDown,
}: EntryRowProps) {
  const title = `${entry.issuer} · ${entry.account}`;
  return (
    <li className="flex items-center gap-2 rounded-lg border p-3">
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm text-muted-foreground">{title}</p>
        <CodeArea
          entry={entry}
          title={title}
          hideCodes={hideCodes}
          onCopy={onCopy}
          onNextCode={onNextCode}
        />
      </div>
      <Button variant="ghost" size="icon" aria-label={`Edit ${title}`} onClick={onEdit}>
        <Pencil />
      </Button>
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
  hideCodes,
  onCopy,
  onNextCode,
}: Pick<EntryRowProps, "entry" | "hideCodes" | "onCopy" | "onNextCode"> & { title: string }) {
  const { revealed, show, hide } = useReveal();
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
      {hideCodes && !revealed ? (
        <>
          <span className="px-2 font-mono text-2xl tracking-wider" aria-label="Code hidden">
            {MASKED_CODE}
          </span>
          <Button variant="outline" size="sm" aria-label={`Show code for ${title}`} onClick={show}>
            <Eye /> Show
          </Button>
          <Button
            variant="ghost"
            size="icon"
            aria-label={`Copy code for ${title}`}
            onClick={() => onCopy(code.text)}
          >
            <Copy />
          </Button>
        </>
      ) : (
        <>
          <Button
            variant="ghost"
            className="font-mono text-2xl tracking-wider"
            aria-label={`Copy code for ${title}`}
            onClick={() => onCopy(code.text)}
          >
            {formatCode(code.text)}
          </Button>
          {hideCodes && (
            <Button variant="ghost" size="icon" aria-label={`Hide code for ${title}`} onClick={hide}>
              <EyeOff />
            </Button>
          )}
        </>
      )}
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
