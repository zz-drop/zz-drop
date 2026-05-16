# Changelog

All notable changes to the `zz-drop`, `zz-drop-core`, and
`zz-drop-tui` crates are recorded here. The three crate
versions move together; this file is the single source of
truth for the workspace.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project follows [Semantic Versioning](https://semver.org/);
the 0.x line is the pre-1.0 stabilisation track. The public
surfaces frozen on the road to 1.0 are listed in
[`AGENTS.md`](AGENTS.md).

## [Unreleased]

## [0.9.1] — 2026-05-16

### Fixed

- **Global flags after the verb** — `--json`, `--quiet`,
  `--passphrase-file`, `--alias`, `--local`, `--remote`,
  `--yes` are now consumed wherever they appear on the
  command line (e.g. `zz f --json`, `zz d note.txt --json`).
  Previously the pre-pass stopped at the first positional, so
  flags placed after the verb were forwarded to the verb
  parser and rejected. The `--` terminator still freezes any
  remaining flags as positionals.
- **`zz dx` bundle extraction** — recognises GNU tar magic
  (`ustar ` at offset 257) in addition to POSIX `ustar\0`, so
  bundles produced by GNU tar are detected and unpacked
  instead of being treated as opaque blobs.

## [0.9.0] — 2026-05-16

First release on the 0.9 stabilisation track. Adds the
scriptable contract and graduates the version from the
`0.0.1-pre.N` development series.

### Added

- **Scriptable mode** — `--json` emits one NDJSON event per
  result on stdout with a stable `v: "1"` schema; `--quiet`
  emits one minimal text line per result. Full contract in
  [`docs/scriptable.md`](docs/scriptable.md); machine-checkable
  JSON Schema in
  [`docs/scriptable/zz-drop-output.v1.json`](docs/scriptable/zz-drop-output.v1.json).
- **Global flags** parsed before the verb: `--json`,
  `--quiet`, `--passphrase-file`, `--alias`, `--local`,
  `--remote`, `--yes`. `--quiet` and `--json` are mutually
  exclusive.
- **Environment overrides** (flag > env > default):
  `ZZ_OUTPUT`, `ZZ_PASSPHRASE_FILE`, `ZZ_ALIAS`,
  `ZZ_CONTAINER`. `ZZ_CONFIG_DIR=<absolute>` redirects the
  whole state tree to `<root>/{config,cache,runtime}`.
- **New exit codes**: `10` (`agent_locked` — scriptable mode
  never auto-unlocks), `11` (`passphrase_file_permissions`).
- **Passphrase-file reader** with strict checks: regular file
  only (no symlinks), owner = current UID, mode ≤ 0600, size
  cap 4 KiB, no embedded NUL, exactly one trailing `\n` stripped.
- **`docs/usage.md`** — scenario cookbook with worked examples
  for every verb.
- **Universal lint test** (`tests/scriptable_universal.rs`)
  asserts every verb either emits ≥ 1 well-formed NDJSON
  record under `--json` or fails with a documented `reason`.

### Changed

- **Doctor (`zz f`) in scriptable mode** now streams one
  `doctor_check` per probe followed by a final
  `doctor_summary` instead of the verbose human output.
- **`zz w`** in `--json` / `--quiet` requires `--yes` (or the
  legacy `ZZ_DROP_CONFIRM_WIPE=yes` env). Refuses with
  `interactive_required` otherwise.
- **`zz c`** in `--json` / `--quiet` fails fast with
  `interactive_only` and exits `2` without launching the TUI.
- **Container resolution**: when both `profiles-local.zz` and
  `profiles-remote.zz` exist and no override was provided,
  scriptable mode fails with `container_ambiguous` instead of
  silently defaulting to local.
- **Alias resolution** in scriptable mode is deterministic
  (flag/env → cached default → single-alias short-circuit →
  `alias_ambiguous` with candidate list). The numbered picker
  never runs under `--json` / `--quiet`.

### Fixed

- **`zz dx`** now correctly extracts tar bundles produced by
  `zz sax` / `zz sarx`. The Rust `tar` crate's default writer
  emits the GNU magic (`ustar `, with trailing space); a
  strict POSIX-only check left the `.tar` on disk un-untar'd.
  `is_tar_ustar` now accepts both POSIX and GNU magic.

### Internal

- Crates bumped from `0.0.1-pre.11` to `0.9.0`.
- `zz-drop-core` adds `scriptable::Reason`,
  `output::json::{Uploaded, Downloaded, Failed, BatchSummary,
  Unlocked, Locked, Wiped, DoctorCheck, DoctorSummary}`, and
  the RFC 3339 helper `now_rfc3339`.
- `zz-drop` (root) adds `runtime` (global-flag pre-pass +
  env merge), `passphrase` (file reader), and the
  `output::emit_*` family that routes results by mode.
- `serde_json = "1"` added as a dev-dependency for the
  integration tests.
