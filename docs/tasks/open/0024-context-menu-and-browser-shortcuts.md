# TASK-0024: The app's own context menu; no browser menu or shortcuts

- Status: in-progress
- Created: 2026-11-27
- Updated: 2026-11-27
- Related ADRs: ADR-0022

## Goal
(User, 2026-11-27, with a screenshot.) On Windows a right-click shows the web browser's menu (Back, Refresh, Save as, Print, Inspect). It must go, and so must the browser's shortcuts. A right-click on an account should offer the app's own actions (the user named show/hide, copy, delete, show secret key). Then install on the user's phone.

## Acceptance criteria
- [ ] No browser menu anywhere except in text boxes (paste must stay)
- [ ] An account's menu: copy code, show/hide code, next code, edit, show secret key, delete; delete and show secret key keep their confirmation and password
- [ ] Browser shortcuts off on Windows; copy and paste keep working; checked in the running app
- [ ] Installed on the phone

## Decisions & notes (append-only, dated)
- 2026-11-27: Tauri 2.12.5 exposes neither of WebView2's two settings. Plan: block the menu in the UI (the native "no menus" setting would also take paste from text boxes) and turn the shortcuts off with WebView2's `AreBrowserAcceleratorKeysEnabled`.

## Progress log (append-only, dated)
- 2026-11-27: Task created.
- 2026-11-27: Built, UI tests 229, backend 147, `cargo deny` ok. Checked in a real WebView2 window (an isolated copy with its own app id, empty vault and test data): the browser menu is blocked on a heading and allowed in the search box; the account menu shows; Delete asks first with Keep focused; Show secret key asks for the device password and shows nothing before it. **Shortcuts not checked:** Windows would not bring the test window to the front, so no keys were sent. Selected menu items beyond these and long press were not tried in a real window.
- 2026-11-27: Found on the way: port 5173 was serving another project, and the first test copy loaded it. `vite.config.ts` now has `strictPort` (the user’s own session uses `tauri.alt-port.conf.json`, port 5174, which overrides the port).

## Next step
1. The user, on Windows: Ctrl+P, F5, Ctrl+R, Ctrl+F, Ctrl+S and Alt+Left do nothing; Ctrl+C, Ctrl+V and Ctrl+A still work in text boxes.
2. Install on the phone (needs a connection), then try long press on an account.
3. Review ADR-0022.
