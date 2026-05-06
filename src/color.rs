use std::borrow::Cow;
use std::collections::HashMap;
use std::io::IsTerminal;

use owo_colors::OwoColorize;

#[derive(Clone, Copy, Debug)]
pub struct ColorPolicy {
    enabled: bool,
}

impl ColorPolicy {
    pub fn detect() -> Self {
        let env = StdEnv;
        let tty = std::io::stdout().is_terminal();
        Self::from_parts(&env, tty)
    }

    pub fn from_parts(env: &dyn EnvLookup, tty: bool) -> Self {
        if let Some(v) = env.get("NO_COLOR")
            && !v.is_empty()
        {
            return Self { enabled: false };
        }
        if env.get("CLICOLOR").as_deref() == Some("0") {
            return Self { enabled: false };
        }
        if let Some(v) = env.get("FORCE_COLOR")
            && !v.is_empty()
        {
            return Self { enabled: true };
        }
        Self { enabled: tty }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn green<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.enabled {
            Cow::Owned(text.green().to_string())
        } else {
            Cow::Borrowed(text)
        }
    }

    pub fn red<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.enabled {
            Cow::Owned(text.red().to_string())
        } else {
            Cow::Borrowed(text)
        }
    }

    pub fn cyan<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.enabled {
            Cow::Owned(text.cyan().to_string())
        } else {
            Cow::Borrowed(text)
        }
    }

    #[cfg(test)]
    pub(crate) fn always() -> Self {
        Self { enabled: true }
    }

    #[cfg(test)]
    pub(crate) fn never() -> Self {
        Self { enabled: false }
    }
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

    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_string(), value.to_string());
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

    fn from(env: MockEnv, tty: bool) -> ColorPolicy {
        ColorPolicy::from_parts(&env, tty)
    }

    #[test]
    fn empty_env_no_tty_off() {
        assert!(!from(MockEnv::empty(), false).enabled());
    }

    #[test]
    fn empty_env_with_tty_on() {
        assert!(from(MockEnv::empty(), true).enabled());
    }

    #[test]
    fn no_color_with_tty_off() {
        assert!(!from(MockEnv::empty().with("NO_COLOR", "1"), true).enabled());
    }

    #[test]
    fn no_color_empty_value_does_not_disable() {
        assert!(from(MockEnv::empty().with("NO_COLOR", ""), true).enabled());
    }

    #[test]
    fn clicolor_zero_off_even_with_tty() {
        assert!(!from(MockEnv::empty().with("CLICOLOR", "0"), true).enabled());
    }

    #[test]
    fn clicolor_one_with_tty_on() {
        assert!(from(MockEnv::empty().with("CLICOLOR", "1"), true).enabled());
    }

    #[test]
    fn clicolor_one_without_tty_still_off() {
        // CLICOLOR=1 means "color is OK if supported", not "force"
        assert!(!from(MockEnv::empty().with("CLICOLOR", "1"), false).enabled());
    }

    #[test]
    fn force_color_overrides_missing_tty() {
        assert!(from(MockEnv::empty().with("FORCE_COLOR", "1"), false).enabled());
    }

    #[test]
    fn no_color_beats_force_color() {
        let env = MockEnv::empty()
            .with("NO_COLOR", "1")
            .with("FORCE_COLOR", "1");
        assert!(!from(env, true).enabled());
    }

    #[test]
    fn cli_color_zero_beats_force_color() {
        let env = MockEnv::empty()
            .with("CLICOLOR", "0")
            .with("FORCE_COLOR", "1");
        assert!(!from(env, true).enabled());
    }

    #[test]
    fn green_passthrough_when_disabled() {
        let p = ColorPolicy::never();
        assert_eq!(&*p.green("ok"), "ok");
    }

    #[test]
    fn green_wraps_with_ansi_when_enabled() {
        let p = ColorPolicy::always();
        let s = p.green("ok");
        assert!(s.contains("\x1b["), "expected ANSI escape, got `{s}`");
        assert!(s.contains("ok"));
    }

    #[test]
    fn red_passthrough_when_disabled() {
        let p = ColorPolicy::never();
        assert_eq!(&*p.red("nope"), "nope");
    }

    #[test]
    fn red_wraps_with_ansi_when_enabled() {
        let p = ColorPolicy::always();
        let s = p.red("nope");
        assert!(s.contains("\x1b["), "expected ANSI escape, got `{s}`");
        assert!(s.contains("nope"));
    }
}
