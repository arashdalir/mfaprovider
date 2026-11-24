import { describe, expect, it } from "vitest";

import { MAX_IMAGE_BYTES, fileToBase64 } from "./file-to-base64";

describe("fileToBase64", () => {
  it("encodes the bytes", async () => {
    expect(await fileToBase64(new File([new Uint8Array([1, 2, 3])], "a.png"))).toBe("AQID");
    expect(await fileToBase64(new File([], "empty.png"))).toBe("");
  });

  it("handles files larger than one conversion chunk", async () => {
    const bytes = new Uint8Array(100_000).map((_, index) => index % 251);
    const decoded = Uint8Array.from(atob(await fileToBase64(new File([bytes], "big.png"))), (c) => c.charCodeAt(0));
    expect(decoded).toEqual(bytes);
  });

  it("refuses files over the limit", async () => {
    const tooBig = new File([new Uint8Array(MAX_IMAGE_BYTES + 1)], "huge.png");
    await expect(fileToBase64(tooBig)).rejects.toThrow(RangeError);
  });
});
