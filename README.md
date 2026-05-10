# zz-drop

[![build](https://github.com/Gibbio/zz-drop/actions/workflows/build.yml/badge.svg)](https://github.com/Gibbio/zz-drop/actions/workflows/build.yml)

Minimalist CLI to put files into — and get files from — a configured
safe cloud destination.

![zz-drop CLI demo](.github/assets/cli-demo.gif)

zz-drop is **not** a sync tool, **not** a mount tool, **not** a generic
cloud file manager. It is a fast, explicit one-shot transfer from a
local machine to a configured cloud target.

## Status

Pre-alpha. v1 ships **local-only** by default — see "v1 ships
local-only" below for what that means and how to opt in to the
remote surface. Daily commands `zz <file>`, `zz d`,
`zz z`, `zz q`, `zz w` are operational against
**Nextcloud** (WebDAV), **Google Drive** (OAuth device flow),
**OneDrive** (OAuth device flow, Microsoft Graph), or **Dropbox**
(OAuth paste-code + PKCE, App folder).
Profiles live inside an encrypted *container* (`profiles-local.zz`)
that can hold many inner profiles; `zz z` unlocks the container
into the local agent and prompts a numbered picker when the
container holds more than one. `zz c` launches the separate
configuration TUI (`zz-tui` binary on PATH).

## Install

To be defined. The release pipeline (signing, install script, Homebrew
formula) is not yet in place.

For a small binary (~1.5 MB) suitable for distribution use the `dist`
profile instead of the default `release`:

```bash
cargo build --profile dist --bin zz-drop
```

`dist` enables `opt-level=z`, full LTO, single codegen unit, symbol
stripping and `panic=abort`. Day-to-day work and CI should stay on
`cargo build --release` (~3× faster compile, slightly larger binary).

### Building with your own OAuth client IDs

The default build embeds public OAuth `client_id` values (and, for
Google Drive, the matching `client_secret`) registered to the
upstream `zz-drop` apps. They are public per the OAuth spec, the
same way [rclone](https://rclone.org) ships its own defaults. Forks
that want their own consent-screen branding, or power users who've
hit the shared per-app rate limits on Google / OneDrive / Dropbox,
can override every value at `cargo build` time without touching the
source:

```bash
ZZ_DROP_GDRIVE_CLIENT_ID="…apps.googleusercontent.com" \
ZZ_DROP_GDRIVE_CLIENT_SECRET="GOCSPX-…" \
ZZ_DROP_ONEDRIVE_CLIENT_ID="…" \
ZZ_DROP_DROPBOX_CLIENT_ID="…" \
cargo build --release
```

Any variable left unset keeps the upstream zz-drop default. Full
table of variables, where to register each app, and a `strings`-based
verification step are in [`docs/build.md`](./docs/build.md).

## Quickstart

```bash
zz <file>...     # upload one or more files (default command)
zz s <file>...   # explicit upload alias
zz d <file>...   # download one or more remote files
zz z             # unlock the active container into the in-RAM agent
zz z local       # force the local container (skip the auto-resolver)
zz q             # lock / clear the agent
zz c             # open the configuration TUI
zz w             # wipe local zz-drop state (typed confirmation)
```

Optional one-off — install the shell completion (bash/zsh/fish):

```bash
zz --completions bash | source                              # bash
zz --completions zsh  > ~/.zfunc/_zz && autoload -U compinit && compinit
zz --completions fish > ~/.config/fish/completions/zz.fish  # fish
```

Pressing TAB after `zz` then offers contextual candidates that
adapt to the current state — local files when you upload, remote
names when you download, sub-directory prefixes for bulk moves.
See [`docs/sacs.md`](docs/sacs.md) for details.

## Examples

A daily session, after a one-off `zz c` to set up at least one
inner profile:

```text
$ zz z
container passphrase:
  [1] casa-nc  (last used)
  [2] gdrive-bright
select [1-2] (Enter for last used): 1
unlocked · casa-nc · cloud.example.org/zz-drop

$ zz readme.md
uploaded readme.md 12 KiB → casa-nc · cloud.example.org/zz-drop

$ zz d leggimi.txt
downloaded leggimi.txt 34 KiB ← casa-nc · cloud.example.org/zz-drop

$ zz q
locked
```

If the agent is locked, every file command says so explicitly:

```text
$ zz readme.md
failed (agent) locked
run: zz z
```

When the container holds a single inner profile the picker is
skipped and `zz z` unlocks straight to it. The cached "default"
alias from `~/.config/zz-drop/last-default-local` (written by the
TUI on the first pick) is the picker's default selection on the
next run — Enter accepts it.

## Commands

| Command | Meaning |
|---|---|
| `zz <file...>` | upload file(s) — primary command |
| `zz s <file...>` | explicit upload alias |
| `zz sx <file>` | upload, compressed (zstd) |
| `zz sa <dir>` | save all top-level files in `<dir>` |
| `zz sar <dir>` | save all, recursively |
| `zz sax <dir>` / `zz sarx <dir>` | bundle `<dir>` as one `.tar.zst` |
| `zz d <file...>` | download remote file(s) |
| `zz dx <file>.zst` | download + decompress (or extract bundle) |
| `zz da <dest>` | download all top-level remote files into `<dest>` |
| `zz dar <dest>` | download all, recursively, into `<dest>` |
| `zz z [local\|remote]` | unlock the active container (picker on N>1) |
| `zz q` | lock / clear the agent's RAM |
| `zz w` | wipe local zz-drop state |
| `zz c` | open the configuration TUI |
| `zz f` | doctor / diagnostics |

The full table with examples, exit codes and parser rules lives
in [`COMMANDS.md`](COMMANDS.md).

### Parser rule

If the first argument exactly matches a reserved command, it is parsed
as that command. To upload a file whose name happens to clash with a
reserved name, prefix it with an explicit path:

```bash
zz ./ls           # uploads a file named "ls"
zz ./sa           # uploads a file named "sa"
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | success |
| `2` | usage error (missing args, unknown command shape) |
| `3` | command recognized but not implemented yet |
| `5` | agent unreachable (socket missing, refused, handshake failed) |
| `6` | profile not found |
| `7` | decryption failed (wrong passphrase or corrupted profile) |
| `8` | wipe cancelled |
| `127` | `zz c` could not find `zz-tui` on PATH (or failed to launch it) |

More codes will be added when the corresponding behavior lands.

## Agent

`zz z` spawns a small per-user local agent in the same binary,
which holds the decrypted container (and the active inner profile)
in RAM under a TTL (10 min) so that later commands don't ask for
the passphrase again. Once locked, the agent self-terminates after
5 minutes of idle.

The agent enforces:

- **per-user socket** under `$XDG_RUNTIME_DIR/zz-drop/` (Linux) or
  `/tmp/zz-drop-$UID/` (macOS)
- **peer UID check** via `SO_PEERCRED` (Linux) / `LOCAL_PEERCRED`
  (macOS) — connections from any other UID are refused
- **token handshake** with a 32-byte token written to a `0600` file;
  comparison is constant-time
- **RAM-only** profile data, no disk persistence, no log file

`zz q` locks. `zz w` wipes local state after typed confirmation.
Full details in
[`docs/agent.md`](docs/agent.md).

## Scope (v1)

In scope:

- explicit one-shot upload / download of local files to/from a single
  configured cloud target
- a small TUI (separate binary `zz-tui`) for configuration only
- a per-user local agent that holds the decrypted profile in RAM only

Out of scope:

- file sync
- mounting cloud storage as a local filesystem
- public share links
- a generic remote file manager
- team / organization access models

### v1 ships local-only — `remote` graduates in v2

Anything that talks to `zz-drop.net` (account login, alias create/list,
container push/pull) is gated behind a Cargo feature `remote`,
default-off. The default `cargo build` produces a binary with no
network code that targets `zz-drop.net`, no DNS resolution toward it,
and no static reference to the host string. To opt in for testing
the remote surface:

```sh
cargo build --features remote
```

The flag is temporary: it graduates default-on in v2 and gets
removed once the v2 line stabilises. See
[`zz-drop-core/docs/feature-flags.md`](../zz-drop-core/docs/feature-flags.md)
for the full inventory.

## Output policy

zz-drop's output is intentionally compact and pipe-safe.

### Format

```text
uploaded readme.md 12 KiB → casa-nc · cloud.example.org/zz-drop
downloaded readme.md 12 KiB ← casa-nc · cloud.example.org/zz-drop
failed   readme.md locked (casa-nc · cloud.example.org/zz-drop)
failed   (agent) locked
run: zz z
```

`uploaded` / `downloaded` and per-file `failed` lines always
include the active alias and a compact target string
(`<host>/<remote-root>`) so the operator can tell at a glance which
destination they hit. Pre-unlock errors (e.g. `(agent) locked`)
have no scope and end with the bare reason.

Sizes use binary prefixes (`B`, `KiB`, `MiB`, `GiB`, `TiB`). Values
under 10 of a prefix get one decimal (`1.5 KiB`), values ≥ 10 are
integer (`12 KiB`).

### Color

Colour is only applied when stdout is a TTY. Standard environment
overrides are honored (precedence top to bottom):

| Variable | Effect |
|---|---|
| `NO_COLOR=<non-empty>` | force off (https://no-color.org) |
| `CLICOLOR=0` | force off |
| `FORCE_COLOR=<non-empty>` | force on, even when piped |
| (none of the above) | on iff stdout is a TTY |

Pipe-safe by default — output to a non-TTY produces no ANSI escapes.

## Files and paths

zz-drop stores its files in OS-appropriate locations, resolved at
runtime via the `directories` crate.

### Linux (XDG)

| Purpose | Path |
|---|---|
| Config | `~/.config/zz-drop/config.toml` |
| Local container (N inner profiles) | `~/.config/zz-drop/profiles-local.zz` |
| Server-synced container (cache) | `~/.config/zz-drop/profiles-remote.zz` (`remote` feature) |
| Cached default alias (local) | `~/.config/zz-drop/last-default-local` |
| Cached default alias (remote) | `~/.config/zz-drop/last-default-remote` (`remote` feature) |
| Agent socket | `$XDG_RUNTIME_DIR/zz-drop/agent.sock` (fallback `/tmp/zz-drop-$UID/agent.sock`) |
| Token | next to the socket |

### macOS

| Purpose | Path |
|---|---|
| Config | `~/Library/Application Support/zz-drop/config.toml` |
| Local container (N inner profiles) | `~/Library/Application Support/zz-drop/profiles-local.zz` |
| Server-synced container (cache) | `~/Library/Application Support/zz-drop/profiles-remote.zz` (`remote` feature) |
| Cached default alias (local) | `~/Library/Application Support/zz-drop/last-default-local` |
| Cached default alias (remote) | `~/Library/Application Support/zz-drop/last-default-remote` (`remote` feature) |
| Agent socket | `/tmp/zz-drop-$UID/agent.sock` |
| Token | `/tmp/zz-drop-$UID/token` |

Both container slots may coexist. `zz z` (and the file commands
that use the agent) pick `profiles-remote.zz` when present, fall
back to `profiles-local.zz` otherwise. `zz z local` / `zz z remote`
override the auto-resolver. `zz w` removes both, the sidecars and
the runtime dir.

### Permissions

- config and runtime directories: `0700`
- `config.toml`, `profiles-local.zz`, `profiles-remote.zz`, `token`: `0600`

### `config.toml`

`config.toml` is plain TOML and **never** contains secrets. Provider
credentials live encrypted inside the profile blob; session tokens
live in the agent's RAM only.

```toml
# zz-drop config — never put secrets here
server_base_url = "https://zz-drop.net"
default_alias = "casa-nc"  # optional
```

A missing config file is treated as defaults. A malformed config
file is a hard error — fix or delete it.

The full schema is documented in
[`zz-drop-core/docs/config.md`](../zz-drop-core/docs/config.md).

## Security

Short version: the cloud provider and any zz-drop API server **must
not** be able to read the decrypted container, the container
passphrase, or the provider credentials inside an inner profile.
Your local user account on your own machine is trusted.

The container is encrypted with Argon2id (`m=190 MiB, t=3, p=1`) +
XChaCha20-Poly1305. The KDF derives a 32-byte KEK from the
operator's passphrase; the agent caches that KEK in
`Zeroizing<[u8; 32]>` for the unlock TTL and uses it to re-encrypt
in place when an inner profile is appended or an OAuth token gets
refreshed — no second passphrase prompt, no nonce reuse (a fresh
24-byte nonce is drawn per encrypt).

When the `remote` feature is enabled and the operator pushes the
container to a `zz-drop.net`-compatible API, the server **does**
see:

- the email and Argon2id-hashed account password
- one or more alias strings the operator chose
- the **encrypted container blob** (opaque ciphertext)
- timestamps + a monotonic version per blob

The server **does not** see:

- the container passphrase
- any inner profile's provider URL, username, app password, OAuth
  token, or remote folder
- any file transferred with `zz s` / `zz d` (those go directly
  to the configured cloud provider, never through the API server)
- the TOTP code typed at login

Local agent keeps the decrypted container in RAM only, behind a
per-user Unix socket with peer-UID + 32-byte token check. No log
file, no telemetry. A pre-alpha hardening pass (cargo audit, secret
log review, agent socket review, profile crypto review) ran on
2026-05-03 — see `zz-drop-core/SECURITY.md` for the headline.

Full details — including honest non-goals (filenames, traffic
timing, forensic erasure) and a vulnerability-disclosure pointer —
live in [`docs/security.md`](docs/security.md). The local agent
specifics are in [`docs/agent.md`](docs/agent.md).

## License

To be confirmed before public release.
