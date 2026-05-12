use std::collections::HashMap;
use std::io::IsTerminal;

use ratatui::style::{Color, Modifier, Style};

// ─── Tokyo-night palette (DESIGN/tokens.css) ────────────────────────

pub const BG_0: Color = Color::Rgb(0x1a, 0x1b, 0x26);
pub const BG_1: Color = Color::Rgb(0x1f, 0x20, 0x30);
pub const BG_2: Color = Color::Rgb(0x24, 0x25, 0x3a);
pub const BG_3: Color = Color::Rgb(0x2c, 0x2e, 0x44);
pub const BG_4: Color = Color::Rgb(0x36, 0x3a, 0x54);
pub const BORDER: Color = Color::Rgb(0x3b, 0x42, 0x61);

pub const FG: Color = Color::Rgb(0xc0, 0xca, 0xf5);
pub const FG_DIM: Color = Color::Rgb(0x82, 0x8b, 0xb8);
pub const FG_MUTE: Color = Color::Rgb(0x54, 0x5c, 0x7e);
pub const FG_BRIGHT: Color = Color::Rgb(0xff, 0xff, 0xff);

pub const CYAN: Color = Color::Rgb(0x7d, 0xcf, 0xff);
pub const BLUE: Color = Color::Rgb(0x82, 0xaa, 0xff);
pub const GREEN: Color = Color::Rgb(0xa3, 0xe6, 0x35);
pub const MINT: Color = Color::Rgb(0x4f, 0xd6, 0xbe);
pub const YELLOW: Color = Color::Rgb(0xff, 0xc7, 0x77);
pub const ORANGE: Color = Color::Rgb(0xff, 0x96, 0x6c);
pub const PINK: Color = Color::Rgb(0xff, 0x9b, 0xd6);
pub const MAGENTA: Color = Color::Rgb(0xc0, 0x99, 0xff);
pub const RED: Color = Color::Rgb(0xff, 0x75, 0x7f);

pub const ACCENT: Color = MINT;
pub const OK: Color = GREEN;
pub const WARN: Color = YELLOW;
pub const DANGER: Color = RED;
pub const INFO: Color = CYAN;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub colored: bool,
}

impl Theme {
    pub fn detect() -> Self {
        let env = StdEnv;
        let tty = std::io::stdout().is_terminal();
        Self::from_parts(&env, tty)
    }

    pub fn from_parts(env: &dyn EnvLookup, tty: bool) -> Self {
        if let Some(v) = env.get("NO_COLOR")
            && !v.is_empty()
        {
            return Self { colored: false };
        }
        if env.get("CLICOLOR").as_deref() == Some("0") {
            return Self { colored: false };
        }
        if let Some(v) = env.get("FORCE_COLOR")
            && !v.is_empty()
        {
            return Self { colored: true };
        }
        Self { colored: tty }
    }

    fn fg(&self, c: Color) -> Style {
        if self.colored {
            Style::default().fg(c)
        } else {
            Style::default()
        }
    }

    // ── plain text styles ────────────────────────────────────────

    pub fn body(&self) -> Style {
        self.fg(FG)
    }

    /// fg-mute — `#545c7e`. Used for hint copy and de-emphasised rows.
    pub fn dim(&self) -> Style {
        if self.colored {
            Style::default().fg(FG_MUTE)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        }
    }

    /// fg-dim — `#828bb8`. Brighter than `dim()`. Used for FormField labels.
    pub fn dim_bright(&self) -> Style {
        if self.colored {
            Style::default().fg(FG_DIM)
        } else {
            Style::default()
        }
    }

    /// fg-bright + BOLD. Used for the "zz-tui" wordmark and screen breadcrumbs.
    pub fn header(&self) -> Style {
        if self.colored {
            Style::default().fg(FG_BRIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    /// Mint accent — used for focused borders and selected radios.
    pub fn accent(&self) -> Style {
        if self.colored {
            Style::default().fg(ACCENT)
        } else {
            Style::default().add_modifier(Modifier::UNDERLINED)
        }
    }

    pub fn accent_bold(&self) -> Style {
        if self.colored {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    pub fn cyan(&self) -> Style {
        if self.colored {
            Style::default().fg(CYAN)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    pub fn yellow(&self) -> Style {
        if self.colored {
            Style::default().fg(YELLOW)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    pub fn ok(&self) -> Style {
        if self.colored {
            Style::default().fg(OK)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    pub fn warn(&self) -> Style {
        if self.colored {
            Style::default().fg(WARN)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    pub fn danger(&self) -> Style {
        if self.colored {
            Style::default().fg(DANGER)
        } else {
            Style::default()
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        }
    }

    // ── chrome ───────────────────────────────────────────────────

    pub fn keybar_chip(&self) -> Style {
        if self.colored {
            Style::default().fg(BG_0).bg(FG_DIM).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        }
    }

    pub fn keybar_label(&self) -> Style {
        self.dim()
    }

    // ── pill ────────────────────────────────────────────────────

    /// Idle / no-profile chip in the title bar — neutral grey bg.
    pub fn pill_idle(&self) -> Style {
        if self.colored {
            Style::default().fg(BG_0).bg(FG_DIM).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        }
    }

    /// Profile-ready chip — mint bg.
    pub fn pill_ready(&self) -> Style {
        if self.colored {
            Style::default().fg(BG_0).bg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        }
    }

    /// Locked chip — yellow bg.
    pub fn pill_warn(&self) -> Style {
        if self.colored {
            Style::default().fg(BG_0).bg(WARN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        }
    }

    /// Running chip — cyan bg.
    pub fn pill_running(&self) -> Style {
        if self.colored {
            Style::default().fg(BG_0).bg(CYAN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        }
    }

    pub fn focus_bg(&self) -> Style {
        if self.colored {
            Style::default().fg(FG_BRIGHT).bg(BG_3)
        } else {
            Style::default().add_modifier(Modifier::REVERSED)
        }
    }

    /// Style for a panel border in the given accent.
    pub fn border_accent(&self, accent: PanelAccent) -> Style {
        let c = match accent {
            PanelAccent::Mint => ACCENT,
            PanelAccent::Cyan => CYAN,
            PanelAccent::Yellow => YELLOW,
            PanelAccent::Red => DANGER,
            PanelAccent::Magenta => MAGENTA,
            PanelAccent::Dim => BORDER,
        };
        if self.colored {
            Style::default().fg(c)
        } else {
            Style::default()
        }
    }

    /// Same colour as `border_accent` but for the title strip text.
    pub fn panel_title(&self, accent: PanelAccent) -> Style {
        let s = self.border_accent(accent);
        if self.colored {
            s.add_modifier(Modifier::BOLD)
        } else {
            s.add_modifier(Modifier::BOLD)
        }
    }

    // ── stepper ─────────────────────────────────────────────────

    pub fn step_active(&self) -> Style {
        if self.colored {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        }
    }

    /// Style for the leading dot glyph of the active step (mint, no underline).
    pub fn step_active_dot(&self) -> Style {
        if self.colored {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    pub fn step_past(&self) -> Style {
        self.dim_bright()
    }

    pub fn step_future(&self) -> Style {
        self.dim()
    }

    pub fn step_disabled(&self) -> Style {
        if self.colored {
            Style::default().fg(BG_4)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelAccent {
    Mint,
    Cyan,
    Yellow,
    Red,
    Magenta,
    Dim,
}

pub trait EnvLookup {
    fn get(&self, key: &str) -> Option<String>;
}

pub struct StdEnv;

impl EnvLookup for StdEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[derive(Default)]
pub struct MockEnv {
    vars: HashMap<String, String>,
}

impl MockEnv {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with(mut self, k: &str, v: &str) -> Self {
        self.vars.insert(k.to_string(), v.to_string());
        self
    }
}

impl EnvLookup for MockEnv {
    fn get(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tty_disables_color() {
        assert!(!Theme::from_parts(&MockEnv::empty(), false).colored);
    }

    #[test]
    fn tty_enables_color() {
        assert!(Theme::from_parts(&MockEnv::empty(), true).colored);
    }

    #[test]
    fn no_color_overrides_tty() {
        let t = Theme::from_parts(&MockEnv::empty().with("NO_COLOR", "1"), true);
        assert!(!t.colored);
    }

    #[test]
    fn force_color_overrides_no_tty() {
        let t = Theme::from_parts(&MockEnv::empty().with("FORCE_COLOR", "1"), false);
        assert!(t.colored);
    }

    #[test]
    fn no_color_beats_force_color() {
        let env = MockEnv::empty()
            .with("NO_COLOR", "1")
            .with("FORCE_COLOR", "1");
        assert!(!Theme::from_parts(&env, true).colored);
    }

    #[test]
    fn no_color_mode_never_emits_bg() {
        let t = Theme::from_parts(&MockEnv::empty(), false);
        // None of the foreground accessors set a background in NO_COLOR mode
        assert!(t.body().bg.is_none());
        assert!(t.dim().bg.is_none());
        assert!(t.accent().bg.is_none());
        assert!(t.cyan().bg.is_none());
        assert!(t.ok().bg.is_none());
        assert!(t.warn().bg.is_none());
        assert!(t.danger().bg.is_none());
        // Chip-style accessors: also no bg in NO_COLOR mode (use REVERSED instead)
        assert!(t.keybar_chip().bg.is_none());
        assert!(t.pill_idle().bg.is_none());
        assert!(t.pill_ready().bg.is_none());
        assert!(t.pill_warn().bg.is_none());
        assert!(t.pill_running().bg.is_none());
    }
}
