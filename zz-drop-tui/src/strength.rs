/// Recommended minimum length per the public TUI design notes.
pub const RECOMMENDED_MIN_LENGTH: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrengthResult {
    pub score: u8,
    pub label: &'static str,
    pub recommended_length_ok: bool,
    pub long_enough: bool,
}

impl StrengthResult {
    /// True when zxcvbn says weak OR the passphrase is shorter than
    /// the recommended minimum. Either condition triggers the "are you
    /// sure?" warning before we cipher the profile.
    pub fn is_weak(&self) -> bool {
        self.score <= 1 || !self.long_enough
    }
}

pub fn evaluate(passphrase: &str) -> StrengthResult {
    if passphrase.is_empty() {
        return StrengthResult {
            score: 0,
            label: "—",
            recommended_length_ok: false,
            long_enough: false,
        };
    }
    let estimate = zxcvbn::zxcvbn(passphrase, &[]);
    let score: u8 = u8::from(estimate.score());
    let label = label_for_score(score);
    let long_enough = passphrase.chars().count() >= RECOMMENDED_MIN_LENGTH;
    StrengthResult {
        score,
        label,
        recommended_length_ok: long_enough,
        long_enough,
    }
}

pub fn label_for_score(score: u8) -> &'static str {
    match score {
        0 => "very weak",
        1 => "weak",
        2 => "fair",
        3 => "good",
        4 => "strong",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_weak() {
        let r = evaluate("");
        assert_eq!(r.score, 0);
        assert!(r.is_weak());
    }

    #[test]
    fn short_strong_phrase_is_still_weak_due_to_length() {
        // "Tr0ub4dor!1" is 11 chars — under recommended 12.
        let r = evaluate("Tr0ub4dor!1");
        // It may score 3+ from zxcvbn, but length triggers weak.
        assert!(!r.long_enough);
        assert!(r.is_weak());
    }

    #[test]
    fn common_password_is_weak() {
        let r = evaluate("password");
        assert!(r.score <= 1);
        assert!(r.is_weak());
    }

    #[test]
    fn long_strong_phrase_is_not_weak() {
        let r = evaluate("correct horse battery staple jaguar 17");
        assert!(r.long_enough);
        assert!(r.score >= 3);
        assert!(!r.is_weak());
    }

    #[test]
    fn label_for_score_matches() {
        assert_eq!(label_for_score(0), "very weak");
        assert_eq!(label_for_score(4), "strong");
        assert_eq!(label_for_score(99), "?");
    }
}
