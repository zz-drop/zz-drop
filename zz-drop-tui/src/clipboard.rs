/// Best-effort clipboard write. Returns `Ok(())` on success and a
/// short reason string on failure — `arboard` may fail on headless
/// systems with no display server, and that must not panic the TUI.
pub fn copy_to_clipboard(text: &str) -> Result<(), &'static str> {
    let mut ctx = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(_) => return Err("clipboard not available"),
    };
    match ctx.set_text(text.to_string()) {
        Ok(()) => Ok(()),
        Err(_) => Err("clipboard write failed"),
    }
}

/// Best-effort browser open. Returns `Ok(())` on success and a short
/// reason string on failure — never automatically opens.
pub fn open_in_browser(url: &str) -> Result<(), &'static str> {
    match open::that_detached(url) {
        Ok(()) => Ok(()),
        Err(_) => Err("could not open browser"),
    }
}
