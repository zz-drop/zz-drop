//! Hand-written NDJSON emitter. The wire schema is one JSON
//! object per line:
//!
//! ```json
//! {"value": "...", "display": "...", "description": "...", "kind": "...", "rank": 1}
//! ```
//!
//! The emitter sorts entries deterministically (rank ascending,
//! then `value` ascending) so snapshot tests cannot flap on
//! HashMap iteration order. The schema is documented for users
//! in `zz-drop/docs/sacs.md`.

use std::fmt::Write;

/// One candidate the shell script will turn into a dropdown row.
/// `value` is what gets inserted into the buffer; `display` is the
/// label shown; `description` is the hint to the right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub value: String,
    pub display: String,
    pub description: String,
    pub kind: Kind,
    /// Lower numbers sort first.
    pub rank: u32,
}

/// One of the eight kinds documented in design §6. The string
/// form on the wire is the lowercase variant name with `_`
/// preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Verb,
    Atomic,
    FileLocal,
    DirLocal,
    FileRemote,
    DirRemote,
    Help,
    Footer,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Verb => "verb",
            Self::Atomic => "atomic",
            Self::FileLocal => "file_local",
            Self::DirLocal => "dir_local",
            Self::FileRemote => "file_remote",
            Self::DirRemote => "dir_remote",
            Self::Help => "help",
            Self::Footer => "footer",
        }
    }
}

/// Render a list of candidates to NDJSON (one object per line).
/// The output is sorted by `rank` ascending, then by `value`
/// ascending — both for determinism in snapshot tests and to
/// match the reading order in the design mockups.
pub fn render(mut candidates: Vec<Candidate>) -> String {
    candidates.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.value.cmp(&b.value)));
    let mut buf = String::with_capacity(candidates.len() * 96);
    for c in &candidates {
        emit_one(&mut buf, c);
        buf.push('\n');
    }
    buf
}

fn emit_one(buf: &mut String, c: &Candidate) {
    buf.push('{');
    write_kv(buf, "value", &c.value);
    buf.push_str(", ");
    write_kv(buf, "display", &c.display);
    buf.push_str(", ");
    write_kv(buf, "description", &c.description);
    buf.push_str(", ");
    buf.push_str("\"kind\": \"");
    buf.push_str(c.kind.as_str());
    buf.push_str("\", ");
    let _ = write!(buf, "\"rank\": {}", c.rank);
    buf.push('}');
}

fn write_kv(buf: &mut String, key: &str, value: &str) {
    buf.push('"');
    buf.push_str(key);
    buf.push_str("\": \"");
    escape_into(buf, value);
    buf.push('"');
}

/// JSON string escaping limited to what the wire actually needs:
/// `"`, `\`, control chars below 0x20, and DEL (0x7F). UTF-8
/// passes through.
fn escape_into(buf: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(buf, "\\u{:04x}", c as u32);
            }
            c => buf.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(value: &str, kind: Kind, rank: u32, description: &str) -> Candidate {
        Candidate {
            value: value.to_string(),
            display: format!("zz {value}"),
            description: description.to_string(),
            kind,
            rank,
        }
    }

    #[test]
    fn empty_input_renders_empty_string() {
        assert_eq!(render(vec![]), "");
    }

    #[test]
    fn single_entry_round_trips() {
        let out = render(vec![cand("c", Kind::Atomic, 1, "open the configuration TUI")]);
        assert_eq!(
            out,
            "{\"value\": \"c\", \"display\": \"zz c\", \"description\": \"open the configuration TUI\", \"kind\": \"atomic\", \"rank\": 1}\n"
        );
    }

    #[test]
    fn entries_are_sorted_by_rank_then_value() {
        let unsorted = vec![
            cand("zb", Kind::Verb, 2, ""),
            cand("aa", Kind::Verb, 1, ""),
            cand("ab", Kind::Verb, 1, ""),
            cand("zc", Kind::Verb, 2, ""),
        ];
        let out = render(unsorted);
        let lines: Vec<&str> = out.lines().collect();
        let values: Vec<&str> = lines
            .iter()
            .map(|l| {
                let start = l.find("\"value\": \"").unwrap() + "\"value\": \"".len();
                let rest = &l[start..];
                let end = rest.find('"').unwrap();
                &rest[..end]
            })
            .collect();
        assert_eq!(values, vec!["aa", "ab", "zb", "zc"]);
    }

    #[test]
    fn quotes_and_backslashes_are_escaped() {
        let out = render(vec![Candidate {
            value: "weird\"name\\".to_string(),
            display: "x".to_string(),
            description: "x".to_string(),
            kind: Kind::Verb,
            rank: 1,
        }]);
        assert!(out.contains(r#""value": "weird\"name\\""#));
    }

    #[test]
    fn newlines_become_escape_sequences() {
        let out = render(vec![Candidate {
            value: "a\nb".to_string(),
            display: "x".to_string(),
            description: "x".to_string(),
            kind: Kind::Verb,
            rank: 1,
        }]);
        // The output must remain a single physical line — embedded
        // \n in `value` is a quoted escape, not a literal break.
        assert_eq!(out.lines().count(), 1);
        assert!(out.contains(r#""value": "a\nb""#));
    }

    #[test]
    fn each_kind_serialises_to_its_design_string() {
        // Locks the wire format against accidental rename.
        for (k, expected) in [
            (Kind::Verb, "verb"),
            (Kind::Atomic, "atomic"),
            (Kind::FileLocal, "file_local"),
            (Kind::DirLocal, "dir_local"),
            (Kind::FileRemote, "file_remote"),
            (Kind::DirRemote, "dir_remote"),
            (Kind::Help, "help"),
            (Kind::Footer, "footer"),
        ] {
            let out = render(vec![cand("x", k, 1, "")]);
            assert!(
                out.contains(&format!("\"kind\": \"{expected}\"")),
                "Kind::{k:?} expected wire form {expected}, got {out}"
            );
        }
    }
}
