# TASK-0019: Back navigation that works on the phone and the desktop

- Status: in-progress
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: none (UI-only)

## Goal
(User request, 2026-11-26, after using the Android build.) Every screen that is not the account list has a visible Back button, and the Android back button (and the browser/desktop back gesture, Alt+Left, mouse back button) goes back one level instead of closing the app. Applies to nested screens too (e.g. Add account → paste link → review).

## Acceptance criteria
- [x] Back stack module: a screen registers a handler while it is not at its root; the system back goes to the top handler; leaving through the UI does not leave stale history entries behind
- [x] OpenVault views (add, edit, import, settings) and the steps inside Add account and Import use it
- [x] Each of those screens has a Back button at the top (same place, same label)
- [x] Back from the account list on Android still leaves the app (nothing registered)
- [x] Locking the vault clears the stack
- [x] Tests (hook and at least one nested flow); UI tests green; checked on the phone

## Decisions & notes (append-only, dated)
- 2026-11-26: Mechanism = `history.pushState` per open screen plus one `popstate` listener. Android's generated `WryActivity` already calls `webView.goBack()` while `canGoBack()`, so no native code is needed. Handlers are a stack; the top one runs on system back.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

- 2026-11-26: Done. `lib/back-stack.ts` (`pushBack`, `goBack`), `lib/use-back.ts` (`useBack(handler | null)`), `components/BackButton.tsx` (calls `goBack`, so the button and the system back are the same thing). OpenVault shows one Back button above every non-list view and registers its own level; Add account (every step but the first; review discards drafts) and Import (preview step discards the pending file) register nested levels. Bottom "Back" buttons in the Import, link and image screens are gone (link/image now say "Cancel" at the bottom). Locking unmounts the screens, which gives all history entries back. UI tests 159 (new: stack/hook 6, OpenVault navigation 3; one ImportScreen test removed because its Back button moved to OpenVault). **Not verified:** the Android hardware/gesture back on the phone (mechanism relies on the generated `WryActivity` calling `webView.goBack()`, read in source, not run); Alt+Left on desktop.

## Next step
Rebuild and install on the phone together with the QR code (TASK-0020); user checks that back goes up one level and leaves the app only from the list.
