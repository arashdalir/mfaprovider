# ADR-0022: No browser menu or browser shortcuts; accounts have their own menu

- Status: proposed
- Date: 2026-11-27
- Related tasks: TASK-0024

## Context
(User, 2026-11-27.) The app's screens are drawn by a webview, which brings a web browser's right-click menu (reload, save as, print, inspect) and shortcuts. In an authenticator, printing or saving the page can copy what is on screen out of the app. Tauri 2.12.5 offers neither of WebView2's settings for this.

## Options considered
1. WebView2's "no context menus" setting. It also removes the menu from text boxes, and with it pasting by mouse. Windows only.
2. **The UI cancels the menu event everywhere but in text boxes.** Works on every platform and can be tested.
3. Shortcuts: cancel their key events in the UI. Needs a list that has to be complete.
4. **Shortcuts: WebView2's `AreBrowserAcceleratorKeysEnabled = false`**, which Microsoft documents as turning off the browser's keys while keeping text editing keys.

## Decision
Options 2 and 4. Accounts get the app's own menu (right-click, or long press on touch screens) from the UI library already in use: copy code, show or hide code, next code, edit, show secret key, delete. "Show secret key" and "Delete" open the editor at that step, so the device password and the confirmation are asked as before; the menu itself deletes and reveals nothing.

Reaching the setting needs one `unsafe` block of two COM calls and, on Windows, direct use of `webview2-com` and `windows-core`, in the versions Tauri already brings.

Also: the development server must get exactly the port the window opens (`strictPort`). Otherwise a debug build shows whatever else serves that port and gives it the app's commands.

## Consequences
+ One menu for mouse and touch; printing and saving are not offered.
- Outside text boxes, selected text can be copied only with the keyboard.
- Text boxes keep the webview's editing menu; what a debug build adds to it (such as Inspect) stays.
- The `unsafe` block and the two crates must follow Tauri's WebView2 version.
- Nothing equivalent was needed or checked for Linux and macOS webviews' shortcuts.
