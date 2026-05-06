# zz-drop local paths and configuration

This document describes where zz-drop stores its files on disk and what
the local `config.toml` may contain.

## Linux (XDG)

| Purpose | Path |
|---|---|
| Config dir | `~/.config/zz-drop/` |
| Config file | `~/.config/zz-drop/config.toml` |
| Profile (encrypted) | `~/.config/zz-drop/profile.zz` |
| Cache dir | `~/.cache/zz-drop/` |
| Runtime dir | `$XDG_RUNTIME_DIR/zz-drop/` (fallback `/tmp/zz-drop-$UID/`) |
| Agent socket | `<runtime>/agent.sock` |
| Token file | `<runtime>/token` |

## macOS

| Purpose | Path |
|---|---|
| Config dir | `~/Library/Application Support/zz-drop/` |
| Config file | `~/Library/Application Support/zz-drop/config.toml` |
| Profile (encrypted) | `~/Library/Application Support/zz-drop/profile.zz` |
| Cache dir | `~/Library/Caches/zz-drop/` |
| Runtime dir | `/tmp/zz-drop-$UID/` |
| Agent socket | `<runtime>/agent.sock` |
| Token file | `<runtime>/token` |

The base directories are resolved via the `directories` crate; the
runtime fallback uses the effective UID of the calling process.

## Permissions (Unix)

Enforced by the reference implementation when the directory or file
is created or saved:

| Path | Mode |
|---|---|
| Config dir | `0700` |
| Runtime dir | `0700` |
| `config.toml` | `0600` |
| `profile.zz` | `0600` (defense in depth, even though it is encrypted) |
| `token` | `0600` |

## `config.toml` schema

`config.toml` is plain TOML. It **never** contains secrets. The
profile decrypt passphrase, provider credentials, OAuth tokens and
session tokens never appear here:

- the profile passphrase lives only in the user's head and is used
  to decrypt `profile.zz`
- provider credentials live encrypted inside `profile.zz`
- session tokens live in RAM (in the agent process) only

```toml
# zz-drop config — never put secrets here
server_base_url = "https://zz-drop.net"
default_alias = "casa-nc"  # optional, last alias used
```

Defaults:

| Field | Default |
|---|---|
| `server_base_url` | `"https://zz-drop.net"` |
| `default_alias` | (omitted) |

A missing config file is treated as defaults. A malformed config
file is a loud error — the user must fix or delete it before
continuing.

## Implementation

The reference implementation lives in `zz-drop-core`:

- `zz_drop_core::config::Paths` — resolved paths for the current run
- `zz_drop_core::config::PathOverrides` — test-time overrides
- `zz_drop_core::config::discover_paths(uid, &PathOverrides)` — pure
  function; it does not call `getuid` itself, so it stays portable
  and easy to test
- `zz_drop_core::config::ensure_dir(path, mode)` — Unix-only,
  creates the directory and applies `mode`
- `zz_drop_core::config::LocalConfig` — TOML schema as a Rust struct
- `zz_drop_core::config::{load, load_or_default, save}` — `save`
  enforces `0600` on Unix

The CLI binary `zz-drop` provides a thin wrapper
(`zz_drop::config::discover()`) that resolves the UID via
`rustix::process::getuid()` before delegating to
`discover_paths`.
