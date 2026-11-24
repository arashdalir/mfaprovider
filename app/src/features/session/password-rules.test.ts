import { describe, expect, it } from "vitest";

import { newPasswordProblem } from "./password-rules";

describe("newPasswordProblem", () => {
  it("rejects short passwords first", () => {
    expect(newPasswordProblem("short", "different")).toMatch(/at least 8/);
  });

  it("rejects a mismatching confirmation", () => {
    expect(newPasswordProblem("long enough", "long enougH")).toMatch(/do not match/);
  });

  it("accepts a long enough matching password", () => {
    expect(newPasswordProblem("long enough", "long enough")).toBeNull();
  });

  it("counts characters, not bytes", () => {
    expect(newPasswordProblem("ééééééé", "ééééééé")).toMatch(/at least 8/);
    expect(newPasswordProblem("éééééééé", "éééééééé")).toBeNull();
  });
});
