import { ChevronDown, ChevronUp, Eye, EyeOff, Pencil } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import type { EntryView } from "@/ipc/client";

import type { StartWith } from "./EntryForm";
import { formatCode } from "./format";

const LOW_SECONDS = 5;
export const REVEAL_SECONDS = 30;
const TOGGLE_WIDTH = "w-20";
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
  /** Opens the account's editor, optionally at one of its actions. */
  onEdit: (startWith?: StartWith) => void;
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
  const reveal = useReveal();
  const masked = hideCodes && !reveal.revealed;
  const toggle = hideCodes ? (masked ? reveal.show : reveal.hide) : undefined;
  return (
    <ContextMenu>
      <ContextMenuTrigger render={<li className="flex items-center gap-2 rounded-lg border p-3" />}>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm text-muted-foreground">{title}</p>
          <CodeArea
            entry={entry}
            title={title}
            masked={masked}
            onToggle={toggle}
            onCopy={onCopy}
            onNextCode={onNextCode}
          />
        </div>
        <Button variant="ghost" size="icon" aria-label={`Edit ${title}`} onClick={() => onEdit()}>
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
      </ContextMenuTrigger>
      <EntryMenu
        entry={entry}
        masked={masked}
        onToggle={toggle}
        onCopy={onCopy}
        onNextCode={onNextCode}
        onEdit={onEdit}
      />
    </ContextMenu>
  );
}

type CodeActions = Pick<EntryRowProps, "entry" | "onCopy" | "onNextCode"> & {
  masked: boolean;
  /** Absent when codes are always shown. */
  onToggle?: () => void;
};

function EntryMenu({
  entry,
  masked,
  onToggle,
  onCopy,
  onNextCode,
  onEdit,
}: CodeActions & Pick<EntryRowProps, "onEdit">) {
  const { code } = entry;
  return (
    <ContextMenuContent>
      {code.state === "ready" && (
        <>
          <ContextMenuItem onClick={() => onCopy(code.text)}>Copy code</ContextMenuItem>
          {onToggle && (
            <ContextMenuItem onClick={onToggle}>{masked ? "Show code" : "Hide code"}</ContextMenuItem>
          )}
          {code.period_seconds === null && (
            <ContextMenuItem onClick={onNextCode}>Next code</ContextMenuItem>
          )}
        </>
      )}
      <ContextMenuItem onClick={() => onEdit()}>Edit…</ContextMenuItem>
      <ContextMenuItem onClick={() => onEdit("reveal")}>Show secret key…</ContextMenuItem>
      <ContextMenuItem destructive onClick={() => onEdit("delete")}>
        Delete…
      </ContextMenuItem>
    </ContextMenuContent>
  );
}

function CodeArea({
  entry,
  title,
  masked,
  onToggle,
  onCopy,
  onNextCode,
}: CodeActions & { title: string }) {
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
        {masked ? MASKED_CODE : formatCode(code.text)}
      </Button>
      {onToggle && (
        <Button
          variant="outline"
          size="sm"
          className={TOGGLE_WIDTH}
          aria-label={`${masked ? "Show" : "Hide"} code for ${title}`}
          onClick={onToggle}
        >
          {masked ? <Eye /> : <EyeOff />} {masked ? "Show" : "Hide"}
        </Button>
      )}
      <div className="flex flex-1 justify-end">
        {remaining !== null && period !== null ? (
          <CountdownRing title={title} remaining={remaining} period={period} />
        ) : (
          <Button variant="outline" size="sm" onClick={onNextCode}>
            Next code
          </Button>
        )}
      </div>
    </div>
  );
}

const RING_SIZE = 40;
const RING_STROKE = 4;
const RING_RADIUS = (RING_SIZE - RING_STROKE) / 2;
const RING_LENGTH = 2 * Math.PI * RING_RADIUS;

function CountdownRing({
  title,
  remaining,
  period,
}: {
  title: string;
  remaining: number;
  period: number;
}) {
  const low = remaining <= LOW_SECONDS;
  const center = RING_SIZE / 2;
  return (
    <div
      role="progressbar"
      aria-label={`Time left for ${title}`}
      aria-valuemin={0}
      aria-valuemax={period}
      aria-valuenow={remaining}
      aria-valuetext={`${remaining} seconds`}
      className="relative size-10 shrink-0"
    >
      <svg width={RING_SIZE} height={RING_SIZE} className="-rotate-90" aria-hidden="true">
        <circle
          cx={center}
          cy={center}
          r={RING_RADIUS}
          fill="none"
          strokeWidth={RING_STROKE}
          className="stroke-muted"
        />
        <circle
          cx={center}
          cy={center}
          r={RING_RADIUS}
          fill="none"
          strokeWidth={RING_STROKE}
          strokeLinecap="round"
          strokeDasharray={RING_LENGTH}
          strokeDashoffset={RING_LENGTH * (1 - remaining / period)}
          className={low ? "stroke-destructive" : "stroke-primary"}
        />
      </svg>
      <span
        aria-hidden="true"
        className="absolute inset-0 flex items-center justify-center text-sm tabular-nums"
      >
        {remaining}
      </span>
    </div>
  );
}
