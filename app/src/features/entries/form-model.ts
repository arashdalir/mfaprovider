import type {
  DraftEditsRequest,
  DraftView,
  EntryChangesRequest,
  EntryView,
  NewEntryRequest,
  OtpTypeView,
  ParamSpecView,
} from "@/ipc/client";

export interface FormValues {
  otpType: string;
  issuer: string;
  account: string;
  /** Typed base32 seed; empty when editing and the secret is to be kept. */
  secret: string;
  params: Record<string, string>;
  /** Comma-separated. */
  tags: string;
  notes: string;
}

export function defaultParams(type: OtpTypeView | undefined): Record<string, string> {
  return Object.fromEntries((type?.params ?? []).map((spec) => [spec.name, spec.default]));
}

export function blankValues(types: OtpTypeView[]): FormValues {
  const type = types.find((candidate) => candidate.id === "totp") ?? types[0];
  return {
    otpType: type?.id ?? "totp",
    issuer: "",
    account: "",
    secret: "",
    params: defaultParams(type),
    tags: "",
    notes: "",
  };
}

export function valuesOf(entry: EntryView, types: OtpTypeView[]): FormValues {
  const type = types.find((candidate) => candidate.id === entry.otp_type);
  return {
    otpType: entry.otp_type,
    issuer: entry.issuer,
    account: entry.account,
    secret: "",
    params: { ...defaultParams(type), ...entry.params },
    tags: entry.tags.join(", "),
    notes: entry.notes,
  };
}

export function valuesOfDraft(draft: DraftView, types: OtpTypeView[]): FormValues {
  const type = types.find((candidate) => candidate.id === draft.otp_type);
  return {
    otpType: draft.otp_type,
    issuer: draft.issuer,
    account: draft.account,
    secret: "",
    params: { ...defaultParams(type), ...draft.params },
    tags: "",
    notes: "",
  };
}

export function toDraftEdits(values: FormValues): DraftEditsRequest {
  return {
    issuer: values.issuer,
    account: values.account,
    params: values.params,
    tags: parseTags(values.tags),
    notes: values.notes,
  };
}

export function parseTags(text: string): string[] {
  return text
    .split(",")
    .map((tag) => tag.trim())
    .filter((tag) => tag !== "");
}

function paramProblem(spec: ParamSpecView, value: string): string | null {
  if (spec.kind.kind === "integer") {
    const number = Number(value);
    const { min, max } = spec.kind;
    return /^\d+$/.test(value) && number >= min && number <= max
      ? null
      : `${spec.name} must be a whole number from ${min} to ${max}.`;
  }
  return spec.kind.choices.includes(value) ? null : `${spec.name} must be one of the listed values.`;
}

/** The first thing wrong with the form, or null. `adding` makes the secret mandatory. */
export function formProblem(
  values: FormValues,
  type: OtpTypeView | undefined,
  adding: boolean,
): string | null {
  if (values.issuer.trim() === "" && values.account.trim() === "") {
    return "Enter an issuer or an account name.";
  }
  if (adding && values.secret.trim() === "") {
    return "Enter the secret key.";
  }
  for (const spec of type?.params ?? []) {
    const problem = paramProblem(spec, values.params[spec.name] ?? spec.default);
    if (problem) {
      return problem;
    }
  }
  return null;
}

export function toNewEntryRequest(values: FormValues): NewEntryRequest {
  return {
    otp_type: values.otpType,
    issuer: values.issuer,
    account: values.account,
    secret_base32: values.secret,
    params: values.params,
    icon: null,
    tags: parseTags(values.tags),
    notes: values.notes,
  };
}

const sameList = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

function sameParams(a: Record<string, string>, b: Record<string, string>): boolean {
  const names = new Set([...Object.keys(a), ...Object.keys(b)]);
  return [...names].every((name) => a[name] === b[name]);
}

/**
 * Only what differs from `baseline` (the values the form started with), so untouched fields,
 * and parameters the form merely filled with defaults, are never rewritten.
 */
export function toChangesRequest(
  baseline: FormValues,
  values: FormValues,
  paramsEditable: boolean,
): EntryChangesRequest {
  const tags = parseTags(values.tags);
  return {
    issuer: values.issuer !== baseline.issuer ? values.issuer : null,
    account: values.account !== baseline.account ? values.account : null,
    secret_base32: values.secret.trim() !== "" ? values.secret : null,
    params: paramsEditable && !sameParams(values.params, baseline.params) ? values.params : null,
    icon: null,
    clear_icon: false,
    tags: sameList(tags, parseTags(baseline.tags)) ? null : tags,
    notes: values.notes !== baseline.notes ? values.notes : null,
  };
}
