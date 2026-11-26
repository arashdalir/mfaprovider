# TASK-0019: Back navigation that works on the phone and the desktop

- Status: in-progress
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: none (UI-only)

## Goal
(User request, 2026-11-26, after using the Android build.) Every screen that is not the account list has a visible Back button, and the Android back button (and the browser/desktop back gesture, Alt+Left, mouse back button) goes back one level instead of closing the app. Applies to nested screens too (e.g. Add account → paste link → review).

## Acceptance criteria
- [ ] Back stack module: a screen registers a handler while it is not at its root; the system back goes to the top handler; leaving through the UI does not leave stale history entries behind
- [ ] OpenVault views (add, edit, import, settings) and the steps inside Add account and Import use it
- [ ] Each of those screens has a Back button at the top (same place, same label)
- [ ] Back from the account list on Android still leaves the app (nothing registered)
- [ ] Locking the vault clears the stack
- [ ] Tests (hook and at least one nested flow); UI tests green; checked on the phone

## Decisions & notes (append-only, dated)
- 2026-11-26: Mechanism = `history.pushState` per open screen plus one `popstate` listener. Android's generated `WryActivity` already calls `webView.goBack()` while `canGoBack()`, so no native code is needed. Handlers are a stack; the top one runs on system back.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

## Next step
Write `app/src/lib/back-stack.ts` + `useBack`, test it, then wire the screens.
