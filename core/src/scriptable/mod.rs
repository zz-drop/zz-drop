//! Scriptable mode (`--json` / `--quiet`) support.
//!
//! Public NDJSON event schema for the CLI's `--json` mode.
//! Schema version is [`SCHEMA_V`]; the field set is stable once
//! 1.0 ships and additive thereafter — breaking changes bump
//! `v` to `"2"`.
//!
//! Event structs live in [`crate::output::json`]; this module owns
//! the schema-version constant and the closed enum of failure
//! reasons, both shared across all events.

pub mod schema;

pub use schema::{Reason, SCHEMA_V};
