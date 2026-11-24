export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;

const CHUNK = 0x8000;

/** The file's bytes as base64, for handing an image to the core. Refuses files over 10 MiB. */
export async function fileToBase64(file: File): Promise<string> {
  if (file.size > MAX_IMAGE_BYTES) {
    throw new RangeError("The file is larger than 10 MiB.");
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + CHUNK));
  }
  return btoa(binary);
}
