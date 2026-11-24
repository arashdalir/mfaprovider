export const CLIPBOARD_CLEAR_MS = 30_000;

let pendingClear: ReturnType<typeof setTimeout> | undefined;

/**
 * Copies `text`, then empties the clipboard after `clearAfterMs` — unless it now holds
 * something else we can see, which is left alone. If the clipboard cannot be read, it is
 * cleared anyway: leaving a code behind is the worse mistake.
 */
export async function copyWithAutoClear(
  text: string,
  clearAfterMs: number = CLIPBOARD_CLEAR_MS,
): Promise<void> {
  await navigator.clipboard.writeText(text);
  clearTimeout(pendingClear);
  pendingClear = setTimeout(() => void clearIfUnchanged(text), clearAfterMs);
}

async function clearIfUnchanged(text: string): Promise<void> {
  try {
    const current = await navigator.clipboard.readText().catch(() => text);
    if (current === text) {
      await navigator.clipboard.writeText("");
    }
  } catch {
    // Clearing is best effort; there is nothing more to do if the write is refused.
  }
}
