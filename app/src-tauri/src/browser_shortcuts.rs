//! Turns off the web browser's own shortcuts, such as print, save, reload and find
//! (ADR-0022). Copying, pasting, selecting and undo keep working.

use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Controller, ICoreWebView2Settings3,
};
use windows_core::Interface;

pub fn disable(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    window.with_webview(|webview| {
        if let Err(error) = turn_off(&webview.controller()) {
            eprintln!("could not turn off the browser shortcuts: {error}");
        }
    })
}

fn turn_off(controller: &ICoreWebView2Controller) -> windows_core::Result<()> {
    // SAFETY: the controller belongs to a live webview, and Tauri runs this on the thread
    // that owns it, which is what these COM calls require.
    unsafe {
        controller
            .CoreWebView2()?
            .Settings()?
            .cast::<ICoreWebView2Settings3>()?
            .SetAreBrowserAcceleratorKeysEnabled(false)
    }
}
