# zz-drop commands

The **canonical** command table, output format and parser rule for the
zz-drop project live upstream in the shared crate, at
[`zz-drop-core/docs/commands.md`](../../zz-drop-core/docs/commands.md).

This file is a thin pointer: keep grammar changes in the canonical
location to avoid drift between the binary and the spec.

User-facing manual with examples:
[`COMMANDS.md`](../COMMANDS.md).

## Implementation in this repository

This binary's parser is implemented in `src/cli/parser.rs` and
exposes `zz_drop::parse_args` to the integration tests in
`tests/cli_parser.rs`.

The grammar in this repository matches the canonical table:

- atom verbs (`q`, `w`, `z`, `c`, `f`)
- composite verbs `s` and `d` with a set of single-letter
  modifiers from `{a, r, x}`; `e` is parsed but rejected with a
  v1.1 message
- if the first argument exactly matches a reserved verb (or its
  composite shape) it is the command; otherwise everything is
  treated as upload paths

## Exit codes

| Code | Meaning |
|---|---|
| `0` | success |
| `2` | usage error (missing args, unknown command shape) |
| `3` | command recognized but not implemented yet |
| `5` | agent unreachable (socket missing, refused, handshake failed) |
| `6` | profile not found |
| `7` | decryption failed (wrong passphrase or corrupted container) |
| `8` | wipe cancelled |
| `9` | provider error or batch with at least one failure |
| `127` | `zz c` could not find or launch the `zz-tui` binary on PATH |

### File commands need an unlocked agent

Every file command resolves the active inner profile by talking
to the local agent. If the agent is not running or the container
is locked, the command prints `failed: locked` + `run: zz z` and
exits with `5`.

### Skip rules

`zz <file>` silently **skips** without failing the batch:

- dotfiles (basename starts with `.`)
- symlinks (no follow)
- directories
- non-regular files (block / char / fifo / socket)

The same rules apply to `zz sa <dir>` / `zz sar <dir>`: dotdirs
are not descended into, symlinks are never followed.

### Batch behavior

- per-file errors do not abort the batch — every file is attempted
- the exit code is `0` if all files succeeded, `9` if at least one
  failed; skipped files don't count as failures
- output uses the formats pinned by the output module:
  ```text
  uploaded   readme.md 12 KiB → casa-nc · cloud.example.org/zz-drop
  downloaded report.pdf 1.5 MiB ← casa-nc · cloud.example.org/zz-drop
  failed     readme.md not found
  ```

### Path semantics

- `zz <file>` — `<file>` is a local path; the remote name is its
  basename. Subdirectories on the local side are not preserved.
- `zz sar <dir>` — preserves the local directory structure on
  the remote.
- `zz d <name>` — `<name>` may include `/`; the file is saved
  with its basename in the current directory.
- `zz z <email>` / `zz z <alias>` — pull the encrypted profile
  container from the configured zz-drop server, persist it as
  `profiles-remote.zz`, then chain into the same unlock dance as
  `zz z remote`. The form with `@` in it is treated as an account
  email; otherwise it is treated as a stored alias (which in v1
  requires a saved session — not yet supported, surfaced as a
  clear error). Server URL is read from `$ZZ_SERVER_URL`. Gated
  behind the `remote` Cargo feature (default-off in v1) — the
  parser accepts the form in every build, the executor surfaces
  "remote not enabled" if the feature is off.
- `zz d <pattern>` — when `<pattern>` contains `*` or `?`, the
  pattern is expanded server-side: zz lists the parent directory
  of the pattern (root if the pattern has no `/`), matches each
  basename against the pattern, and downloads every match. The
  shell can't help here — its glob engine only sees the local
  filesystem — so the operator must keep the pattern out of the
  shell's globbing reach (quote it: `zz d 'Q*'`, or install the
  per-shell wrapper documented in
  [`sacs.md`](./sacs.md#per-shell-wrapper-for-zz-d-pattern-download-glob)).
  Path-segment globs (`*` before the last `/`) are not supported
  in v1; brackets / character classes are also not supported.
- `zz dar <dest>` — mirrors the remote tree under `<dest>`.

### Collision policy

The current default policy is `Rename`: the first uploaded file
keeps its name; subsequent uploads with the same basename get a
unique suffix (format pinned in `zz-drop-core::providers::nextcloud::collision::rename_with_suffix`).
Per-profile collision policy is selectable from the TUI and
sealed into the encrypted container.
