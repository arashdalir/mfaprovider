/**
 * The system "back" (Android's button, Alt+Left, a mouse's back button) goes through the
 * browser history. Every open screen that is not at its root registers a handler here and
 * owns one history entry; back runs the handler registered last.
 *
 * Closing a screen from the UI gives its history entry back, so the next system back does
 * not hit a stale entry. Entries are given back in a batch after the current task, so a
 * screen that closes and another that opens at once reuse the same entry.
 */
type Handler = () => void;

const handlers: Handler[] = [];
let ignoredPops = 0;
let entriesToGiveBack = 0;
let listening = false;

function onPopState() {
  if (ignoredPops > 0) {
    ignoredPops -= 1;
    return;
  }
  handlers.pop()?.();
}

function giveBackEntries() {
  const count = entriesToGiveBack;
  entriesToGiveBack = 0;
  if (count > 0) {
    ignoredPops += 1;
    window.history.go(-count);
  }
}

/** Registers `handler` for the system back; the returned function removes it again. */
export function pushBack(handler: Handler): () => void {
  if (!listening) {
    window.addEventListener("popstate", onPopState);
    listening = true;
  }
  if (entriesToGiveBack > 0) {
    entriesToGiveBack -= 1;
  } else {
    window.history.pushState(null, "");
  }
  handlers.push(handler);
  return () => {
    const index = handlers.lastIndexOf(handler);
    if (index === -1) {
      return;
    }
    handlers.splice(index, 1);
    entriesToGiveBack += 1;
    queueMicrotask(giveBackEntries);
  };
}

/** What a visible Back button does: exactly what the system back does. */
export function goBack() {
  window.history.back();
}

/** Forgets everything; for tests. */
export function resetBackStack() {
  handlers.length = 0;
  entriesToGiveBack = 0;
  ignoredPops = 0;
}
