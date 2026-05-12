# zz-drop

[![build](https://github.com/zz-drop/zz-drop/actions/workflows/build.yml/badge.svg)](https://github.com/zz-drop/zz-drop/actions/workflows/build.yml)
[![release](https://img.shields.io/github/v/release/zz-drop/zz-drop?include_prereleases&sort=semver)](https://github.com/zz-drop/zz-drop/releases)
[![license](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue)](#license)

Minimalist CLI to put files into — and get files from — a
configured safe cloud destination. Four providers, one command,
end-to-end encryption on every credential the tool stores.

![zz-drop CLI demo](.github/assets/cli-demo.gif)

zz-drop is **not** a sync tool, **not** a mount tool, **not** a
generic cloud file manager. One-shot uploads, one-shot downloads,
explicit, fast.

## Install

```bash
# macOS / Linux (Homebrew tap) — installs both `zz-drop` (CLI)
# and `zz-tui` (configuration TUI) in one shot.
brew install zz-drop/zz-drop/zz-drop

# Or curl-installer (signed binaries, no Homebrew needed)
curl -fsSL https://github.com/zz-drop/zz-drop/releases/latest/download/zz-drop-installer.sh | sh
```

Every release artifact is signed with [minisign](https://jedisct1.github.io/minisign/);
the public key is [`release-key.pub`](release-key.pub).

Build from source: see [`docs/build.md`](docs/build.md).

## Quickstart

```bash
zz c            # one-off setup (configuration TUI)
zz file.md      # upload
zz d file.md    # download
zz z            # unlock the agent for the session
zz q            # lock the agent
```

```text
$ zz readme.md
uploaded readme.md 12 KiB → casa-nc · cloud.example.org/zz-drop

$ zz d leggimi.txt
downloaded leggimi.txt 34 KiB ← casa-nc · cloud.example.org/zz-drop
```

Output always names the active alias and the destination. Sizes
are binary (`KiB` / `MiB` / `GiB`). Colors only on a TTY, with
`NO_COLOR` / `CLICOLOR=0` honored.

## What you get

- **Four destinations**: Nextcloud (WebDAV), Google Drive
  (OAuth device flow), OneDrive (OAuth device flow + Microsoft
  Graph), Dropbox (OAuth paste-code + PKCE, App folder).
- **Encrypted profile container** (`profile-local.zz`):
  XChaCha20-Poly1305 + Argon2id; passphrase never leaves the
  device. The server side (when used in v2) sees only an
  opaque encrypted blob.
- **Local per-user agent** in the same binary: holds the
  decrypted profile in RAM only, TTL 10 min, idle locked-exit
  after 5 min, Unix socket bound per-UID.
- **State-aware shell completion** (SACS): `zz d <TAB>` shows
  your remote files; `zz s <TAB>` shows local ones;
  `zz z <TAB>` shows your inner profiles. zsh / bash / fish.
- **Composable verb grammar** for power users:
  `zz sx file.md` (zstd compress), `zz sa dir/` (bulk
  top-level), `zz sar dir/` (recursive), `zz sarx dir/`
  (recursive + tar.zst). The `d` family mirrors it.

## TUI

The configuration TUI is a separate binary, `zz-tui`, shipped
in the same release tarball as `zz-drop`. `zz c` exec's it.

![zz-drop TUI walkthrough](.github/assets/tui-demo.gif)

Use the TUI to:

- onboard a cloud destination (provider OAuth, Nextcloud Login
  Flow, or Nextcloud app-password)
- set or rotate the master passphrase
- add / remove inner profiles inside the container
- run diagnostics, push / recover a remote profile copy

The TUI is **setup-only**. Daily file moves stay in the CLI.

## Scope (v1)

In: explicit one-shot upload / download, one configured cloud
target per profile, the local agent, the configuration TUI.

Out: file sync, mount-as-FS, public share links, generic remote
file manager, team / org access.

v1 ships **local-only** by default. The account-side surface
(login on `zz-drop.net`, profile push / recover) graduates from
the `remote` Cargo feature in v2 — the default `cargo build`
has no `zz-drop.net` strings, no DNS resolution toward it, no
network code that targets it. See
[`docs/feature-flags.md`](docs/feature-flags.md).

## Documentation

- [`COMMANDS.md`](COMMANDS.md) — every command, examples,
  exit codes, parser rule
- [`SECURITY.md`](SECURITY.md) — threat model, server-side
  guarantees and non-claims, supported crypto
- [`docs/build.md`](docs/build.md) — build from source, BYO
  OAuth client IDs, shell completion install
- [`docs/agent.md`](docs/agent.md) — agent protocol, socket
  layout, token handshake, lock / TTL semantics
- [`docs/sacs.md`](docs/sacs.md) — state-aware completion
  internals
- [`docs/file-encryption.md`](docs/file-encryption.md) — `.zzd`
  blob format for client-side file E2EE (v1.1)
- [`docs/profile-format.md`](docs/profile-format.md) —
  `profile.zz` envelope and payload schema
- [`docs/providers/`](docs/providers/) — per-provider notes
  (Nextcloud, Google Drive, OneDrive, Dropbox)

## License

Dual: [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE) at your
option.
