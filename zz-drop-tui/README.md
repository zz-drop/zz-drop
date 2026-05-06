# zz-drop-tui

Ratatui-based setup and configuration UI for the
[zz-drop](https://zz-drop.net) project.

`zz-tui` is **not** the daily interface — daily upload / download lives
in the CLI (`zz file.md`). This binary is for setup, provider
configuration, profile passphrase entry and recovery.

## Status

Pre-alpha. v1 ships **local-only** by default — see the section
"v1 ships local-only" below for what that means and how to opt in
to the remote surface.

The Welcome screen exposes a **LOCAL** track:

- *Create local container* runs the wizard (provider → server URL
  / OAuth → auth → remote folder → collision policy → test upload
  → passphrase) and writes the encrypted container as
  `profiles-local.zz` with a strong "no recovery if file or
  passphrase lost" warning.
- *Add profile to local container* (shown when the file already
  exists) unlocks the existing container and routes the wizard to
  append a new inner profile, re-encrypting in place against the
  cached KEK — the operator does not retype the passphrase.
- *Open local container* shows the picker; the operator chooses
  any inner profile to inspect or re-test.

Supported providers in v1: **Nextcloud** (WebDAV with App Password
or Login Flow v2) and **Google Drive** (OAuth 2.0 device flow).

When built with `--features remote`, a parallel **REMOTE** track
appears on the Welcome screen for `profiles-remote.zz` (the local
cache of a server-synced container) — *Add to remote container*,
*Sign in & fetch*, *Open synced container*.

The session token and most-recently-active inner profile are
cached in RAM for the duration of a `zz-tui` run, so re-opening
the same source or re-pushing skips the passphrase / login
screens.

Detailed flow: [`docs/setup.md`](docs/setup.md). Login Flow
specifics: [`docs/login-flow.md`](docs/login-flow.md). Profile
passphrase: [`docs/profile-passphrase.md`](docs/profile-passphrase.md).

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

| Screen | Status |
|---|---|
| Welcome | implemented (two-track LOCAL / REMOTE menu) |
| Provider | implemented (Nextcloud only in v1) |
| Nextcloud server | implemented (URL validation) |
| Nextcloud auth — app password | implemented (with masked input) |
| Nextcloud auth — Login Flow | implemented (URL / inline-image QR / ASCII fallback / clipboard / browser-on-request) |
| Remote folder | implemented (path validation via core) |
| Collision policy | implemented (Rename / Overwrite / Fail) |
| Test upload | implemented (four-stage probe with live progress: `ensure folder` → `marker` → `upload` → `cleanup`) |
| Profile passphrase + persistence | implemented (zxcvbn strength meter, weak warning, encrypts `profiles-local.zz`, file mode 0600) |
| Done | implemented (final summary + CLI cheat sheet; `↵` returns to Welcome, `q` exits) |
| Profile unlock | implemented (passphrase prompt for `profiles-local.zz` or `profiles-remote.zz`; honours session cache) |
| Profile manage | implemented (view fields, reveal app password, re-test, re-push, wipe) |
| Account login (push / sign-in sub-flow) | implemented (email + password; reuses cached session token across screens in the same run) |
| Two-factor (TOTP) | implemented (only when account has 2FA on) |
| Push / Download profile | implemented (alias picker; in wizard push the blob is re-encrypted with the picked alias before upload so file and server agree on the alias inside the blob; in SignIn mode the same screen downloads the picked alias into `profiles-remote.zz`) |

## Setup flow

A typical run looks like this. You launch `zz-tui` (or `zz c` from
the CLI), pick a track on the **Welcome** screen, walk through the
setup steps, and end on a **Done** screen. Everything is
keyboard-only; the bottom keybar always shows the bindings that
apply right now. The pill on the top right shows whether the
current session is operating on a local or remote profile (with the
server label and alias when known): `o no profile`, `* local · <alias>`,
or `* remote · <server> · <alias>`.

### Welcome

```text
| zz-tui  ›  welcome                                                                                      o no profile


                                                    zz-drop  ·  setup
                                              Multi-cloud file transfer CLI

| › get started
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ ▍ LOCAL   this machine only · no recovery if lost                                                                    │
│  > Create local container   new profiles-local.zz · pick a provider                                                   │
│                                                                                                                      │
│    Quit                                                                                                              │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
                profiles live in ~/.config/zz-drop/. encrypted at rest. server cannot decrypt.
 ↑↓  select    ↵  confirm    q  quit
```

(With `--features remote` a REMOTE block also appears with
*Add to remote container* and *Sign in to zz-drop.net*; the
default v1 build hides it.)

When `profiles-local.zz` and/or `profiles-remote.zz` are already on
disk, an **Open local profile** / **Open synced profile** entry
shows up at the top of each section. Picking it asks for the
passphrase the first time and reuses the in-RAM cache afterwards.

### Nextcloud — Login Flow (with QR)

For headless setups: the server returns an `otpauth://`-style URL,
the TUI shows it as a QR (Kitty/WezTerm/Ghostty native, half-block
ASCII everywhere else), and you authorize from a phone or any
second device with a browser.

```text
| zz-tui  ›  setup › auth › login flow                                                                    o no profile
* welcome  * provider  * server  @ auth  o folder  o encrypt  . push  o done
| authorize with phone                             | link
┌───────────────────────────────────────────────┐  ┌───────────────────────────────────────────────┐
│ █▀▀▀▀▀█  █▀███ ▄▀▄█▀▄ ▄▀█ █▀▀▀▀▀█             │  │  authorization url                            │
│ █ ███ █ █▄█▀█▄▄█▀▄  ▀▀█ ▄ █ ███ █             │  │  https://nextcloud.example.org · press u for  │
│ █ ▀▀▀ █ █ ▀█▄ ▄▄▀█▀▀▀ ▄▀█ █ ▀▀▀ █             │  │  status                                       │
│ ▀▀▀▀▀▀▀ █ ▀ █ █ █ █▄▀▄▀▄█ ▀▀▀▀▀▀▀             │  │  v poll endpoint  reachable                   │
│ █▄▀▀▀█▀▄▄▀█▄ ██▀▄▀▄▄▄▄▀▄▀ ██▀██ ▄             │  │  * user grant  awaiting                       │
│   …  (truncated for the README)               │  │                                               │
└───────────────────────────────────────────────┘  └───────────────────────────────────────────────┘
 c  copy    o  open    u  url detail    i  ascii qr    q  hide qr    esc  back
```

### Done — pushed (happy path)

The pill flips from `no profile` to `local · <alias>` / `remote ·
<server> · <alias>` as soon as the wizard saves a blob.

```text
| zz-tui  ›  done                                                                                    * local · casa-nc

| done
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                                                      │
│   ✓ setup complete                                                                                                   │
│                                                                                                                      │
│   the profile is now retrievable from any shell with                                                                 │
│     zz z casa-nc    # 4096 bytes, version 1                                                                          │
│                                                                                                                      │
│   profile at:  /home/alice/.config/zz-drop/profiles-local.zz                                                          │
│                                                                                                                      │
│   daily use stays in the CLI:                                                                                        │
│                                                                                                                      │
│     zz x          # unlock the profile in the agent                                                                  │
│     zz file.md    # upload                                                                                           │
│     zz d file.md  # download                                                                                         │
│     zz q          # lock                                                                                             │
│                                                                                                                      │
│   press ↵ to go back, q to exit.                                                                                     │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 ↵  back to welcome    q  exit
```

### Done — local-only (no recovery warning)

After **Create local container** with the push step skipped, the
panel turns yellow and spells out exactly what "no recovery" means
plus how to make the profile recoverable later.

```text
| zz-tui  ›  done                                                                                    * local · casa-nc

| local profile saved · no recovery
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│   ⚠  profile saved locally — not on any server                                                                       │
│                                                                                                                      │
│   if you lose either                                                                                                 │
│     • the file profiles-local.zz                                                                                      │
│     • the passphrase you just typed                                                                                  │
│   the contents are gone — there is no recovery.                                                                      │
│                                                                                                                      │
│   to make it recoverable from any shell:                                                                             │
│     • re-run zz-tui, choose Open local profile,                                                                      │
│       then press p to push it to zz-drop.net under an alias.                                                         │
│   …                                                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 ↵  back to welcome    q  exit
```

The full screen catalogue with per-screen keybars and transitions is
in [`docs/screens.md`](docs/screens.md).

## Ratatui notes

`zz-tui` runs entirely in the terminal — no GUI, no `xdg-open`, no
embedded browser. Some constraints we hold ourselves to:

- Target frame **100 × 30**; minimum **80 × 24**. Smaller terminals
  show a centered "terminal too small" message instead of panicking.
- No floating elements outside panel rectangles. No popovers; modals
  (e.g. the Login Flow URL detail) take over the body region.
- Long URLs and paths are truncated with a middle ellipsis or shown
  in a dedicated modal — never wrapped onto an unpredictable number
  of lines.
- The bottom **keybar is always visible** and reflects the current
  state of the screen.
- **Color is never required for meaning**. With `NO_COLOR=1` the
  TUI degrades to `BOLD` / `DIM` / `UNDERLINED` / `REVERSED` and
  every status remains readable.
- **Login Flow is headless-friendly**: a phone is enough to complete
  authentication; no browser on the box running `zz-tui` is required.
- The QR renderer falls back to half-block ASCII whenever inline
  graphics aren't reliably available (see
  [QR rendering](#qr-rendering) above).

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
[`../zz-drop/docs/security.md`](../zz-drop/docs/security.md). The
local agent specifics (which `zz-tui` does not run, but its CLI
sibling does) are in
[`../zz-drop/docs/agent.md`](../zz-drop/docs/agent.md).

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
[`zz-drop-core/docs/feature-flags.md`](../zz-drop-core/docs/feature-flags.md).

## License

To be confirmed before public release.
