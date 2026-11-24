import { describe, expect, it } from "vitest";

import { MAX_TEXT_FILE_BYTES, readTextFile } from "./read-text-file";

const file = (bytes: number[] | Uint8Array) => new File([new Uint8Array(bytes)], "export.txt");

function utf16(text: string, littleEndian: boolean): number[] {
  const bytes = littleEndian ? [0xff, 0xfe] : [0xfe, 0xff];
  for (const unit of text) {
    const code = unit.charCodeAt(0);
    bytes.push(...(littleEndian ? [code & 0xff, code >> 8] : [code >> 8, code & 0xff]));
  }
  return bytes;
}

describe("readTextFile", () => {
  it("reads UTF-8, with or without a byte-order mark", async () => {
    expect(await readTextFile(file(new TextEncoder().encode("café\r\nb")))).toBe("café\r\nb");
    expect(await readTextFile(file([0xef, 0xbb, 0xbf, 0x61]))).toBe("a");
  });

  it("reads UTF-16 in both byte orders and drops the mark", async () => {
    expect(await readTextFile(file(utf16("ab é", true)))).toBe("ab é");
    expect(await readTextFile(file(utf16("ab é", false)))).toBe("ab é");
  });

  it("refuses bytes that are not text", async () => {
    await expect(readTextFile(file([0xc3, 0x28]))).rejects.toThrow("not readable text");
  });

  it("refuses files over the limit without reading them", async () => {
    const huge = new File([new Uint8Array(MAX_TEXT_FILE_BYTES + 1)], "huge.txt");
    await expect(readTextFile(huge)).rejects.toThrow(RangeError);
  });

  it("reads an empty file as empty text", async () => {
    expect(await readTextFile(file([]))).toBe("");
  });
});
