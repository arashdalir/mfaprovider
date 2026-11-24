import { useEffect, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useApi } from "@/ipc/api-context";
import type { EntryView, OtpTypeView, ParamSpecView } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

import { RevealSecret } from "./RevealSecret";
import {
  blankValues,
  defaultParams,
  formProblem,
  toChangesRequest,
  toNewEntryRequest,
  valuesOf,
  type FormValues,
} from "./form-model";
import { typeLabel } from "./type-labels";

interface EntryFormProps {
  /** The account to edit; absent when adding. */
  entry?: EntryView;
  onDone: () => void;
  onCancel: () => void;
}

/** Add or edit one account. Parameter fields come from the type's schema (see the core). */
export function EntryForm(props: EntryFormProps) {
  const api = useApi();
  const [types, setTypes] = useState<OtpTypeView[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .listOtpTypes()
      .then((list) => !cancelled && setTypes(list))
      .catch((error: unknown) => !cancelled && setLoadError(describeError(error)));
    return () => {
      cancelled = true;
    };
  }, [api]);

  if (types === null) {
    return loadError ? (
      <p role="alert" className="text-destructive">
        {loadError}
      </p>
    ) : (
      <p className="text-muted-foreground">Loading…</p>
    );
  }
  return <FormBody {...props} types={types} />;
}

function FormBody({ entry, types, onDone, onCancel }: EntryFormProps & { types: OtpTypeView[] }) {
  const api = useApi();
  const adding = entry === undefined;
  const baseline = entry ? valuesOf(entry, types) : blankValues(types);
  const [values, setValues] = useState<FormValues>(baseline);
  const [showSecret, setShowSecret] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const type = types.find((candidate) => candidate.id === values.otpType);
  const supported = type !== undefined;

  function change(patch: Partial<FormValues>) {
    setValues((current) => ({ ...current, ...patch }));
  }

  async function perform(action: () => Promise<unknown>) {
    setBusy(true);
    try {
      await action();
      onDone();
    } catch (error) {
      setProblem(describeError(error));
      setBusy(false);
    }
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const found = formProblem(values, type, adding);
    setProblem(found);
    if (found) {
      return;
    }
    if (entry) {
      void perform(() => api.editEntry(entry.id, toChangesRequest(baseline, values, supported)));
    } else {
      void perform(() => api.addEntry(toNewEntryRequest(values)));
    }
  }

  return (
    <form noValidate className="flex flex-col gap-4" onSubmit={submit} aria-label={adding ? "Add account" : "Edit account"}>
      <h2 className="text-lg font-semibold">{adding ? "Add account" : "Edit account"}</h2>

      {adding ? (
        <div className="flex flex-col gap-2">
          <Label htmlFor="otp-type">Type</Label>
          <select
            id="otp-type"
            className="rounded-md border bg-background px-2 py-2"
            value={values.otpType}
            onChange={(event) =>
              change({
                otpType: event.target.value,
                params: defaultParams(types.find((t) => t.id === event.target.value)),
              })
            }
          >
            {types.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>
                {typeLabel(candidate.id)}
              </option>
            ))}
          </select>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">Type: {typeLabel(values.otpType)}</p>
      )}

      <TextField id="issuer" label="Issuer" value={values.issuer} onChange={(issuer) => change({ issuer })} />
      <TextField id="account" label="Account" value={values.account} onChange={(account) => change({ account })} />

      <div className="flex flex-col gap-2">
        <Label htmlFor="secret">
          {adding ? "Secret key" : "New secret key (leave empty to keep the current one)"}
        </Label>
        <div className="flex gap-2">
          <Input
            id="secret"
            type={showSecret ? "text" : "password"}
            autoComplete="off"
            spellCheck={false}
            value={values.secret}
            onChange={(event) => change({ secret: event.target.value })}
          />
          <Button type="button" variant="outline" onClick={() => setShowSecret((shown) => !shown)}>
            {showSecret ? "Hide" : "Show"}
          </Button>
        </div>
      </div>

      {entry && <RevealSecret entryId={entry.id} />}

      {supported ? (
        (type?.params ?? []).map((spec) => (
          <ParamField
            key={`${values.otpType}-${spec.name}`}
            spec={spec}
            value={values.params[spec.name] ?? spec.default}
            onChange={(value) => change({ params: { ...values.params, [spec.name]: value } })}
          />
        ))
      ) : (
        <p className="text-sm">
          This type is not supported by this version, so only its name, tags and notes can change.
        </p>
      )}

      <TextField id="tags" label="Tags (comma-separated)" value={values.tags} onChange={(tags) => change({ tags })} />
      <div className="flex flex-col gap-2">
        <Label htmlFor="notes">Notes</Label>
        <textarea
          id="notes"
          className="min-h-20 rounded-md border bg-background px-3 py-2 text-sm"
          value={values.notes}
          onChange={(event) => change({ notes: event.target.value })}
        />
      </div>

      {problem && (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        <Button type="submit" disabled={busy}>
          {adding ? "Add" : "Save"}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        {entry && !confirmingDelete && (
          <Button type="button" variant="destructive" className="ml-auto" onClick={() => setConfirmingDelete(true)}>
            Delete…
          </Button>
        )}
      </div>

      {entry && confirmingDelete && (
        <div role="alertdialog" aria-label="Confirm delete" className="flex flex-col gap-2 rounded-md border border-destructive p-3">
          <p className="text-sm">
            Delete {entry.issuer} · {entry.account}? Its secret is removed from this vault. This
            cannot be undone.
          </p>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="destructive"
              disabled={busy}
              onClick={() => void perform(() => api.deleteEntry(entry.id))}
            >
              Delete permanently
            </Button>
            <Button type="button" variant="outline" onClick={() => setConfirmingDelete(false)}>
              Keep
            </Button>
          </div>
        </div>
      )}
    </form>
  );
}

function TextField({
  id,
  label,
  value,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      <Label htmlFor={id}>{label}</Label>
      <Input id={id} value={value} onChange={(event) => onChange(event.target.value)} />
    </div>
  );
}

function ParamField({
  spec,
  value,
  onChange,
}: {
  spec: ParamSpecView;
  value: string;
  onChange: (value: string) => void;
}) {
  const id = `param-${spec.name}`;
  const label = spec.name.charAt(0).toUpperCase() + spec.name.slice(1);
  return (
    <div className="flex flex-col gap-2">
      <Label htmlFor={id}>{label}</Label>
      {spec.kind.kind === "choice" ? (
        <select
          id={id}
          className="rounded-md border bg-background px-2 py-2"
          value={value}
          onChange={(event) => onChange(event.target.value)}
        >
          {spec.kind.choices.map((choice) => (
            <option key={choice} value={choice}>
              {choice}
            </option>
          ))}
        </select>
      ) : (
        <Input
          id={id}
          type="number"
          inputMode="numeric"
          min={spec.kind.min}
          max={spec.kind.max}
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
      )}
    </div>
  );
}
