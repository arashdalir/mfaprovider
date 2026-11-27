const TEXT_INPUT_TYPES = new Set(["text", "password", "search", "number", "email", "url", "tel"]);

/**
 * Stops the webview from showing a web browser's menu (print, save, reload…) in `target`. Text
 * boxes keep it, because pasting needs it (ADR-0022). Returns a function that undoes this.
 */
export function suppressBrowserMenu(target: Document): () => void {
  const suppress = (event: Event) => {
    if (!isTextBox(event.target)) {
      event.preventDefault();
    }
  };
  target.addEventListener("contextmenu", suppress);
  return () => target.removeEventListener("contextmenu", suppress);
}

function isTextBox(element: EventTarget | null): boolean {
  if (element instanceof HTMLTextAreaElement) {
    return !element.disabled;
  }
  return (
    element instanceof HTMLInputElement && !element.disabled && TEXT_INPUT_TYPES.has(element.type)
  );
}
