//! Mnemonic alias generator for the PushProfile screen.
//!
//! Produces lowercase `<adjective>-<noun>` strings drawn from two
//! curated word lists. The full charset is `[a-z-]`, well within the
//! API's `^[a-z0-9._-]{4,32}$` alias rule, with a small numeric
//! suffix (2 digits) to make collisions unlikely without bloating
//! the typed length.
//!
//! Not a security primitive — this just makes the operator's life
//! easier when picking a fresh alias on the server. Randomness comes
//! from `getrandom`, but if `getrandom` ever fails (which would
//! itself indicate something seriously wrong with the host), we fall
//! back to a deterministic suggestion built from `SystemTime` so the
//! TUI keeps working.

const ADJECTIVES: &[&str] = &[
    "amber", "azure", "balmy", "blithe", "bold", "brave", "breezy",
    "brisk", "calm", "chirpy", "clever", "cosmic", "cozy", "crisp",
    "cyan", "dapper", "dewy", "dusky", "fancy", "feisty", "fluffy",
    "frosty", "gentle", "ginger", "golden", "happy", "hardy", "heady",
    "hearty", "honey", "humble", "icy", "ivory", "jazzy", "jolly",
    "keen", "lacy", "lemon", "lively", "loyal", "lucky", "merry",
    "mighty", "minty", "misty", "mossy", "nimble", "perky", "plucky",
    "quiet", "rosy", "rusty", "shiny", "silver", "silky", "sleek",
    "smooth", "snowy", "soft", "speedy", "spry", "sunny", "swift",
    "tidy", "tiny", "true", "vivid", "warm", "wise", "witty", "zippy",
];

const NOUNS: &[&str] = &[
    "alder", "atlas", "aurora", "badger", "beacon", "birch", "blossom",
    "boulder", "bramble", "brook", "canyon", "cascade", "cedar",
    "cinder", "clover", "comet", "coral", "cove", "crane", "creek",
    "crocus", "crystal", "cypress", "dawn", "delta", "dune", "eddy",
    "ember", "falcon", "fern", "field", "finch", "fjord", "forest",
    "frost", "garnet", "geyser", "glade", "globe", "gorge", "grove",
    "harbor", "harvest", "haven", "heath", "hill", "horizon", "island",
    "ivy", "jade", "kelp", "lagoon", "lark", "linden", "marsh", "meadow",
    "mountain", "nebula", "oak", "oasis", "orchid", "otter", "peak",
    "pine", "plains", "raven", "reef", "ridge", "river", "shore",
    "spruce", "tide", "thicket", "thistle", "tundra", "valley", "vine",
    "willow",
];

/// Provider tag prepended to the alias when the operator is creating
/// a fresh inner profile in the TUI. The short form makes it obvious
/// at a glance which connection a given alias points to (`nc-amber-
/// brook-42` is clearly Nextcloud, `gdrive-…` clearly Google Drive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderPrefix {
    Nextcloud,
    GoogleDrive,
    OneDrive,
    ProtonDrive,
    Dropbox,
}

impl ProviderPrefix {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Nextcloud => "nc",
            Self::GoogleDrive => "gdrive",
            Self::OneDrive => "onedrive",
            Self::ProtonDrive => "proton",
            Self::Dropbox => "dropbox",
        }
    }
}

/// Generate a fresh alias suggestion without provider tag. Returns
/// something like `silver-otter-42`. Always lowercase, always
/// conforms to the `[a-z0-9._-]{4,32}` alias pattern.
pub fn suggest_alias() -> String {
    let (adj_idx, noun_idx, n) = pick_indexes();
    format_suggestion(None, adj_idx, noun_idx, n)
}

/// Generate a fresh alias suggestion with a provider prefix —
/// e.g. `nc-silver-otter-42`. The total length stays within the
/// 32-char alias cap.
pub fn suggest_alias_for(prefix: ProviderPrefix) -> String {
    let (adj_idx, noun_idx, n) = pick_indexes();
    format_suggestion(Some(prefix.tag()), adj_idx, noun_idx, n)
}

fn pick_indexes() -> (usize, usize, usize) {
    let mut buf = [0u8; 4];
    if getrandom::getrandom(&mut buf).is_err() {
        // Deterministic fallback, never returns the same value twice
        // in the same second; good enough for a UX nudge.
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        return (
            (now_nanos as usize) % ADJECTIVES.len(),
            (now_nanos as usize / 17) % NOUNS.len(),
            (now_nanos as usize) % 100,
        );
    }
    let adj_idx = (u32::from_le_bytes([buf[0], buf[1], 0, 0]) as usize) % ADJECTIVES.len();
    let noun_idx = (u32::from_le_bytes([buf[2], buf[3], 0, 0]) as usize) % NOUNS.len();
    let n = (buf[0] as u16 + buf[3] as u16) as usize % 100;
    (adj_idx, noun_idx, n)
}

fn format_suggestion(prefix: Option<&str>, adj_idx: usize, noun_idx: usize, n: usize) -> String {
    let adj = ADJECTIVES[adj_idx];
    let noun = NOUNS[noun_idx];
    // Cap total length at 32 chars (the alias maximum). The longest
    // word combos are ~17 chars; suffix `-NN` adds 3; the longest
    // provider tag (`onedrive`) adds 9 chars + 1 dash → 30 max.
    let s = match prefix {
        Some(p) => format!("{p}-{adj}-{noun}-{n:02}"),
        None => format!("{adj}-{noun}-{n:02}"),
    };
    debug_assert!(s.len() <= 32, "alias too long: `{s}`");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_valid_alias(s: &str) -> bool {
        let len = s.chars().count();
        if !(4..=32).contains(&len) {
            return false;
        }
        s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
    }

    #[test]
    fn always_within_charset_and_length() {
        for _ in 0..200 {
            let s = suggest_alias();
            assert!(is_valid_alias(&s), "invalid alias generated: `{s}`");
        }
    }

    #[test]
    fn contains_two_dashes_and_a_numeric_suffix() {
        let s = suggest_alias();
        assert_eq!(s.matches('-').count(), 2, "shape: `{s}`");
        let suffix = s.rsplit('-').next().unwrap();
        assert_eq!(suffix.len(), 2);
        assert!(suffix.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn at_least_some_variety_in_a_batch() {
        let mut set = std::collections::HashSet::new();
        for _ in 0..30 {
            set.insert(suggest_alias());
        }
        // Lower-bound: 30 calls should give at least 15 unique results
        // out of ~720 000 combinations. If we end up below, the RNG
        // is broken.
        assert!(set.len() >= 15, "only {} unique in 30 calls", set.len());
    }

    #[test]
    fn provider_prefix_carries_provider_tag() {
        for (prefix, tag) in [
            (ProviderPrefix::Nextcloud, "nc-"),
            (ProviderPrefix::GoogleDrive, "gdrive-"),
            (ProviderPrefix::OneDrive, "onedrive-"),
            (ProviderPrefix::ProtonDrive, "proton-"),
            (ProviderPrefix::Dropbox, "dropbox-"),
        ] {
            let alias = suggest_alias_for(prefix);
            assert!(
                alias.starts_with(tag),
                "{:?} should yield `{tag}…`, got `{alias}`",
                prefix
            );
            assert!(is_valid_alias(&alias), "invalid alias generated: `{alias}`");
        }
    }

    #[test]
    fn provider_prefix_alias_stays_within_32_chars() {
        // Stress: generate many onedrive-prefixed aliases (longest tag).
        for _ in 0..200 {
            let alias = suggest_alias_for(ProviderPrefix::OneDrive);
            assert!(
                alias.len() <= 32,
                "alias `{alias}` exceeded 32 chars ({})",
                alias.len()
            );
        }
    }

    #[test]
    fn provider_prefix_alias_has_three_dashes_and_numeric_suffix() {
        let alias = suggest_alias_for(ProviderPrefix::Nextcloud);
        assert_eq!(alias.matches('-').count(), 3, "shape: `{alias}`");
        let suffix = alias.rsplit('-').next().unwrap();
        assert_eq!(suffix.len(), 2);
        assert!(suffix.chars().all(|c| c.is_ascii_digit()));
    }
}
