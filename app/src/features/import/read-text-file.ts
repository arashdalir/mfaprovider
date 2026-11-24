export const MAX_TEXT_FILE_BYTES = 5 * 1024 * 1024;

/**
 * The file's text. UTF-8 and UTF-16 (with a byte-order mark, as Windows editors write
 * "Unicode") are understood; anything else is refused instead of guessed at.
 */
export async function readTextFile(file: File): Promise<string> {
  if (file.size > MAX_TEXT_FILE_BYTES) {
    throw new RangeError("The file is larger than 5 MiB.");
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  const label = bytes[0] === 0xff && bytes[1] === 0xfe
    ? "utf-16le"
    : bytes[0] === 0xfe && bytes[1] === 0xff
      ? "utf-16be"
      : "utf-8";
  try {
    return new TextDecoder(label, { fatal: true }).decode(bytes);
  } catch {
    throw new RangeError("The file is not readable text.");
  }
}
