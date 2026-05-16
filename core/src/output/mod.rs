//! Output-side helpers shared across binaries.
//!
//! Currently exports [`json`], the NDJSON event serializers for
//! the CLI's `--json` mode. Text-mode rendering stays in the
//! `zz-drop` root crate (`src/output.rs`) because it's CLI-specific
//! (ANSI coloring, terminal sizing).

pub mod json;

/// Re-export of `serde::Serialize` so consumer crates (the CLI
/// binary) can write generic helpers over event structs without
/// taking a direct `serde` dependency themselves.
pub use serde::Serialize;
