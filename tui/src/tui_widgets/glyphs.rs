//! Glyph constants used across TUI primitives. When `Theme::colored` is
//! false we degrade to ASCII so the rendering stays legible on terminals
//! that don't ship a font with these box-drawing / Unicode symbols.

pub const BAR_LEFT: char = '▍';      // title-bar left rule
pub const CURSOR: char = '▎';        // form-field cursor
pub const FOCUS: char = '▶';         // radio / nav focus arrow
pub const BLOCK_FULL: char = '█';
pub const BLOCK_LIGHT: char = '░';
pub const HALF_TOP: char = '▀';
pub const HALF_BOT: char = '▄';

pub const CHECK_OK: char = '✔';
pub const CHECK_WARN: char = '!';
pub const CHECK_ERR: char = '✗';
pub const CHECK_SKIP: char = '–';
pub const CHECK_BUSY: char = '◌';

/// Pick the active glyph based on theme. When colour is on we use the
/// design's Unicode glyphs; otherwise we fall back to safe ASCII.
pub fn focus(colored: bool) -> char {
    if colored { FOCUS } else { '>' }
}

pub fn cursor(colored: bool) -> char {
    if colored { CURSOR } else { '|' }
}

pub fn bar_left(colored: bool) -> char {
    if colored { BAR_LEFT } else { '|' }
}

pub fn block_full(colored: bool) -> char {
    if colored { BLOCK_FULL } else { '#' }
}

pub fn block_light(colored: bool) -> char {
    if colored { BLOCK_LIGHT } else { '.' }
}

pub fn check_glyph(colored: bool, status: CheckStatusGlyph) -> char {
    use CheckStatusGlyph::*;
    if colored {
        match status {
            Ok => CHECK_OK,
            Warn => CHECK_WARN,
            Err => CHECK_ERR,
            Skip => CHECK_SKIP,
            Busy => CHECK_BUSY,
        }
    } else {
        match status {
            Ok => 'v',
            Warn => '!',
            Err => 'x',
            Skip => '-',
            Busy => '*',
        }
    }
}

/// Used by `glyphs::check_glyph`. Kept here (instead of inside
/// `check.rs`) to avoid a cross-module enum import for a tiny mapping.
#[derive(Clone, Copy, Debug)]
pub enum CheckStatusGlyph {
    Ok,
    Warn,
    Err,
    Skip,
    Busy,
}
