import { describe, expect, it } from "vitest";

import type { EntryView, OtpTypeView } from "@/ipc/client";

import {
  blankValues,
  formProblem,
  parseTags,
  toChangesRequest,
  toNewEntryRequest,
  valuesOf,
} from "./form-model";

const totp: OtpTypeView = {
  id: "totp",
  params: [
    { name: "algorithm", kind: { kind: "choice", choices: ["SHA1", "SHA256", "SHA512"] }, default: "SHA1" },
    { name: "digits", kind: { kind: "integer", min: 6, max: 8 }, default: "6" },
    { name: "period", kind: { kind: "integer", min: 1, max: 3600 }, default: "30" },
  ],
};
const steam: OtpTypeView = { id: "steam", params: [] };
const types = [steam, totp];

const entry: EntryView = {
  id: "1",
  otp_type: "totp",
  issuer: "GitHub",
  account: "alice",
  icon: null,
  tags: ["work", "mail"],
  notes: "note",
  params: { digits: "8" },
  created_at_ms: 0,
  updated_at_ms: 0,
  code: { state: "unsupported" },
};

describe("blank and existing values", () => {
  it("starts a new account as TOTP with the schema defaults", () => {
    const values = blankValues(types);
    expect(values.otpType).toBe("totp");
    expect(values.params).toEqual({ algorithm: "SHA1", digits: "6", period: "30" });
  });

  it("fills an existing account, completing missing parameters from defaults", () => {
    const values = valuesOf(entry, types);
    expect(values.params).toEqual({ algorithm: "SHA1", digits: "8", period: "30" });
    expect(values.tags).toBe("work, mail");
    expect(values.secret).toBe("");
  });

  it("copes with a type the schema does not know", () => {
    const values = valuesOf({ ...entry, otp_type: "vendor-x", params: { a: "1" } }, types);
    expect(values.params).toEqual({ a: "1" });
  });
});

describe("parseTags", () => {
  it("splits on commas and drops blanks", () => {
    expect(parseTags(" work , ,mail,")).toEqual(["work", "mail"]);
    expect(parseTags("")).toEqual([]);
  });
});

describe("formProblem", () => {
  const good = { ...blankValues(types), account: "me", secret: "JBSWY3DP" };

  it("accepts a complete form", () => {
    expect(formProblem(good, totp, true)).toBeNull();
  });

  it("needs a name and, when adding, a secret", () => {
    expect(formProblem({ ...good, account: " " }, totp, true)).toMatch(/issuer or an account/);
    expect(formProblem({ ...good, secret: " " }, totp, true)).toMatch(/secret key/);
    expect(formProblem({ ...good, secret: "" }, totp, false)).toBeNull();
  });

  it("checks parameters against the schema", () => {
    const withParams = (params: Record<string, string>) => ({ ...good, params });
    expect(formProblem(withParams({ ...good.params, digits: "9" }), totp, true)).toMatch(/digits/);
    expect(formProblem(withParams({ ...good.params, period: "0" }), totp, true)).toMatch(/period/);
    expect(formProblem(withParams({ ...good.params, period: "3.5" }), totp, true)).toMatch(/period/);
    expect(formProblem(withParams({ ...good.params, algorithm: "MD5" }), totp, true)).toMatch(/algorithm/);
  });
});

describe("requests", () => {
  it("builds a new-entry request", () => {
    const values = { ...blankValues(types), issuer: "I", account: "a", secret: "JBSWY3DP", tags: "x, y" };
    expect(toNewEntryRequest(values)).toEqual({
      otp_type: "totp",
      issuer: "I",
      account: "a",
      secret_base32: "JBSWY3DP",
      params: { algorithm: "SHA1", digits: "6", period: "30" },
      icon: null,
      tags: ["x", "y"],
      notes: "",
    });
  });

  it("sends nothing for an untouched edit", () => {
    const baseline = valuesOf(entry, types);
    const request = toChangesRequest(baseline, baseline, true);
    expect(request.issuer).toBeNull();
    expect(request.account).toBeNull();
    expect(request.secret_base32).toBeNull();
    expect(request.tags).toBeNull();
    expect(request.notes).toBeNull();
    expect(request.params).toBeNull();
  });

  it("sends only the fields that changed", () => {
    const baseline = valuesOf(entry, types);
    const values = { ...baseline, account: "bob", tags: "work", secret: "JBSWY3DP" };
    const request = toChangesRequest(baseline, values, true);
    expect(request.account).toBe("bob");
    expect(request.tags).toEqual(["work"]);
    expect(request.secret_base32).toBe("JBSWY3DP");
    expect(request.issuer).toBeNull();
    expect(request.notes).toBeNull();
  });

  it("sends parameters only if they changed and are editable", () => {
    const baseline = valuesOf(entry, types);
    const changed = { ...baseline, params: { algorithm: "SHA1", digits: "6", period: "30" } };
    expect(toChangesRequest(baseline, changed, true).params).toEqual(changed.params);
    expect(toChangesRequest(baseline, changed, false).params).toBeNull();
  });
});
