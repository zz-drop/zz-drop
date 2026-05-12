# zz-drop-tui

Ratatui-based setup and configuration UI for the
[zz-drop](https://zz-drop.net) project.

`zz-tui` is **not** the daily interface — daily upload / download lives
in the CLI (`zz file.md`). This binary is for setup, provider
configuration, profile passphrase entry and recovery.

![zz-drop TUI walkthrough](https://raw.githubusercontent.com/zz-drop/zz-drop/main/.github/assets/tui-demo.gif)

## Status

Pre-alpha. v1 ships **local-only** by default (see "v1 ships
local-only" below). Supported providers: **Nextcloud** (WebDAV with
App Password or Login Flow v2), **Google Drive** (OAuth 2.0 device
flow), **OneDrive** (OAuth device flow, Microsoft Graph), and
**Dropbox** (OAuth paste-code + PKCE, App folder sandbox). Full
setup catalogue, per-screen behavior and recovery semantics live
in [`docs/setup.md`](docs/setup.md),
[`docs/login-flow.md`](docs/login-flow.md), and
[`docs/profile-passphrase.md`](docs/profile-passphrase.md).

## Quickstart

```bash
zz-tui      # standalone
zz c        # via the zz-drop CLI; resolves zz-tui on PATH
```

For a small binary (~3.6 MB) suitable for distribution use the `dist`
profile:

```bash
cargo build --profile dist --bin zz-tui
```

The default `cargo build --release` produces an ~11 MB binary that
compiles ~3× faster — keep it for dev and CI.

The CLI does not embed the TUI — `zz c` is a thin launcher that
exec's the `zz-tui` binary on PATH. If installation didn't put
`zz-tui` on your PATH, run it directly by absolute path or symlink
it yourself.

## Keybindings

Common bindings shared across screens:

| Key | Action |
|---|---|
| `Enter` | confirm / advance |
| `Esc` | back, or quit on the welcome screen |
| `↑` `↓` | move between options (radios, menu items) |
| `Tab` / `Shift+Tab` | cycle between fields (auth, passphrase) |
| `Ctrl+C` | quit anywhere |
| `q` | quit on screens without text input; on text-input screens it's typed as a literal `q` |

The keybar at the bottom of each screen shows the bindings that apply
right now. On the Login Flow screen specifically: `c` copy URL · `o`
open in browser · `u` show full URL · `q` toggle QR · `i` toggle
inline/ASCII QR · `r` retry on failure.

## Terminal requirements

| | Width × Height |
|---|---|
| Target | 100 × 30 |
| Minimum | 80 × 24 |

If the terminal is smaller than the minimum, `zz-tui` does not panic:
it shows a centered "terminal too small" message until the user
resizes the window.

## QR rendering

On the Nextcloud Login Flow screen `zz-tui` shows a QR for the
authorization URL so a phone (or any second device) can complete the
flow in a browser. The renderer is picked automatically:

- **Inline image (Kitty graphics)** — used on terminals known to
  render the protocol quietly:
  - **Kitty** (`$KITTY_WINDOW_ID` set, or `$TERM=xterm-kitty`)
  - **WezTerm** (`$TERM_PROGRAM=WezTerm`)
  - **Ghostty** (`$TERM_PROGRAM=ghostty`, or `$GHOSTTY_RESOURCES_DIR` set)
- **Half-block ASCII** — everywhere else. Works on every terminal,
  the pane is sized so the QR keeps a 1:1 aspect ratio, and a phone
  camera scans it without trouble.

iTerm2 is deliberately *not* on the allowlist: inline images work but
iTerm2 prompts ("Allow this terminal to display a file?") on every
session unless you click "Always allow" first. Apple Terminal has no
inline-image support at all.

Overrides:

| Variable | Effect |
|---|---|
| `ZZ_DROP_TUI_INLINE_QR=1` | force inline regardless of allowlist (e.g. iTerm2 after enabling "Always allow") |
| `ZZ_DROP_TUI_NO_INLINE_QR=1` | force ASCII even on Kitty/WezTerm/Ghostty (beats `INLINE_QR` if both are set) |

When inline is on, press **`i`** on the Login Flow screen to toggle
back to ASCII at runtime without restarting.

## Color policy

Standard environment overrides are honored, in this order:

| Variable | Effect |
|---|---|
| `NO_COLOR=<non-empty>` | force off (https://no-color.org) |
| `CLICOLOR=0` | force off |
| `FORCE_COLOR=<non-empty>` | force on |
| (none of the above) | on iff stdout is a TTY |

When color is off, contrast is preserved with `BOLD`, `DIM`,
`UNDERLINED`, `REVERSED` modifiers — color is never required to
convey meaning.

## Screens

Default build (v1):

| Screen | Status |
|---|---|
| Welcome | implemented (LOCAL track; REMOTE track shown only with `--features remote`) |
| Provider | implemented (Nextcloud, Google Drive, OneDrive, Dropbox) |
| Nextcloud server | implemented (URL validation) |
| Nextcloud auth — app password | implemented (masked input) |
| Nextcloud auth — Login Flow | implemented (URL / inline QR / ASCII fallback / clipboard / browser-on-request) |
| Google Drive auth | implemented (OAuth device flow, scope `drive.file`) |
| OneDrive auth | implemented (OAuth device flow, Microsoft Graph) |
| Dropbox auth | implemented (OAuth paste-code + PKCE, App folder sandbox) |
| Remote folder | implemented (path validation via core) |
| Collision policy | implemented (Rename / Overwrite / Fail) |
| Test upload (probe) | implemented (four-stage: ensure folder → marker → upload → cleanup) |
| Profile passphrase | implemented (strength meter, weak-passphrase warning) |
| Done | implemented (summary + CLI cheat sheet) |
| Profile unlock | implemented (passphrase prompt, session cache) |
| Profile manage | implemented (view fields, reveal credentials, re-test, re-push, wipe) |

Behind `--features remote` (v2 preview, hidden in default build):

| Screen | Status |
|---|---|
| Account login (push / sign-in) | implemented (email + password; session cached in RAM) |
| Two-factor (TOTP) | implemented (only when account has 2FA on) |
| Push / Download profile | implemented (alias picker) |

Per-screen details, keybars and transitions live in
[`docs/screens.md`](docs/screens.md).

## Setup flow

A typical run: launch `zz-tui` (or `zz c` from the CLI), pick a
track on the Welcome screen, walk through provider / auth / folder
/ collision / probe / passphrase, end on a Done screen. Everything
is keyboard-only; the bottom keybar always shows the current
bindings. The pill on the top right reports session state:
`o no profile`, `* local · <alias>`, or `* remote · <server> · <alias>`.

Three distinctive moments:

- **Nextcloud Login Flow with QR.** Headless-friendly auth: the
  TUI renders the authorization URL as a QR (Kitty / WezTerm /
  Ghostty inline graphics, half-block ASCII everywhere else). You
  scan from a phone, approve, the TUI auto-advances.
- **Done — pushed (happy path).** Pill flips to `* local · <alias>`.
  Screen shows the path of `profiles-local.zz`, the alias to use
  with `zz z <alias>`, and the CLI cheat sheet (`zz z`, `zz <file>`,
  `zz d <file>`, `zz q`).
- **Done — local-only (no recovery warning).** Yellow panel: if you
  lose the file or the passphrase, the profile is gone. Explains
  how to make it recoverable later by pushing to `zz-drop.net`.

Full screen catalogue with per-screen keybars and transitions:
[`docs/screens.md`](docs/screens.md). Implementation constraints
on the Ratatui side: [`docs/ratatui-notes.md`](docs/ratatui-notes.md).

## Security

Short version: `zz-tui` is the **setup** UI. The wizard collects what
goes into the profile blob, encrypts it locally with Argon2id +
XChaCha20-Poly1305, and writes the result with mode `0600`. Provider
credentials and the profile-decrypt passphrase **never leave the
device** through `zz-tui`.

Two slots live in the config dir, never both writing the same file:

- `profiles-local.zz` — local-only blob. If you lose either the file
  or the passphrase, the contents are gone (no recovery). The
  Welcome and Save screens spell that out before you commit.
- `profiles-remote.zz` — local cache of an alias on a server. The
  same encrypted bytes also live on the server (push), so even if
  you lose the local file you can fetch it back with `zz z <alias>`
  on any machine, as long as you remember the passphrase.

When you push to a `zz-drop.net`-compatible API (auto on **Create
remote profile**, opt-in on **Create local container**, manual via
**Open profile → re-push**):

- the encrypted blob travels as-is — the server stores opaque
  ciphertext and never sees provider URLs, app passwords, or the
  decrypt passphrase
- account password is sent over TLS and verified server-side; only
  Argon2id hashes are stored
- TOTP is consumed at login when the account opted in; the shared
  seed and recovery codes never reach the client
- the session token returned by login is **cached in RAM** for the
  rest of the `zz-tui` run so re-pushing or fetching another alias
  in the same session doesn't re-prompt; it is wiped on `q` and on
  the wipe action

Project-wide details, including honest non-goals and the
vulnerability-disclosure pointer, live in
[zz-drop/docs/security.md](https://github.com/zz-drop/zz-drop/blob/main/docs/security.md).
The local agent specifics (which `zz-tui` does not run, but its CLI
sibling does) are in
[zz-drop/docs/agent.md](https://github.com/zz-drop/zz-drop/blob/main/docs/agent.md).

## Scope

In scope:
- setup wizard
- provider configuration
- Nextcloud Login Flow UI (headless-friendly)
- profile passphrase screen
- profile push / recover screens (gated behind the `remote` feature)
- account / alias management UI (gated behind the `remote` feature)
- diagnostics UI

Out of scope:
- daily upload / download (use the CLI)
- duplicate crypto / profile-format / API DTO logic (lives in the
  shared crate)
- assuming a graphical desktop is available

### v1 ships local-only — `remote` graduates in v2

Default builds (`cargo build`) produce a TUI with the REMOTE block
in the welcome menu hidden, no API client compiled in, and no
`zz-drop.net` string statically referenced in the binary. To preview
the v2 surface:

```sh
cargo build --features remote
```

The flag is temporary; it graduates default-on in v2 and is removed
once the v2 line stabilises. See
[core/docs/feature-flags.md](https://github.com/zz-drop/core/blob/main/docs/feature-flags.md).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
