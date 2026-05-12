# AGENTS.md — zz-drop monorepo

This repository is the **zz-drop monorepo**. Consolidated 2026-05-12
from three former separate repos (`zz-drop`, `zz-drop-core`,
`zz-drop-tui`). It hosts a Cargo workspace with three members:

- `.` — the `zz-drop` CLI binary + local per-user agent (single
  binary).
- `core/` — the `zz-drop-core` shared library (crypto, profile
  format, provider clients, agent protocol types, API DTOs).
  Crate name kept as `zz-drop-core`; only the subdirectory was
  renamed.
- `tui/` — the `zz-drop-tui` setup/configuration TUI. Ships the
  `zz-tui` binary invoked by `zz c`. Crate name kept as
  `zz-drop-tui`; only the subdirectory was renamed.

zz-drop is a minimalist CLI-first tool to quickly put files into
— and get files from — a configured safe cloud destination.

```bash
zz file.md      # upload
zz d file.md    # download
```

The project is **not** a sync tool, not a mount tool, not a
generic cloud file manager.

## Required reading before editing

Before modifying anything in this repository, read:

- `README.md`
- `SECURITY.md`
- `CONTRIBUTING.md` if present
- relevant files under `docs/`
- this `AGENTS.md`

If the maintainer has provided additional project context for the
session, follow it. If unsure, stop and ask the maintainer before
editing — do not proceed on partial context.

## Per-crate scope

### `.` (zz-drop binary)

Ships the **CLI binary `zz-drop`** (with `zz` symlink when
available) and the **local per-user agent**, in the same binary.

Contains:

- command parser (atomic verbs `s`/`d`/`a`/`r`, composite SACS
  variants, exit code table)
- upload/download/list operations
- agent internal mode
- Unix socket client/server
- profile unlock flow + alias / container resolution
- local wipe
- operational provider clients (Nextcloud WebDAV, Google Drive,
  OneDrive, Dropbox; Proton Drive deferred to v1.1+)
- SACS state-aware completion / suggestions
- minimal launcher for the `zz-tui` binary via `zz c`

Must NOT contain:

- full TUI implementation (lives in `tui/`)
- web dashboard
- billing
- server DB logic
- marketing web pages

Depends on `zz-drop-core` (workspace member at `core/`).

### `core/` (zz-drop-core library)

Shared crate for stable common logic.

Contains:

- profile data types
- `profile.zz` envelope format
- encrypt/decrypt profile functions
- Argon2id parameter model
- XChaCha20-Poly1305 profile crypto
- local agent protocol types
- public API DTOs
- shared error types
- provider model types
- command-independent config/path models

Must NOT contain:

- Ratatui UI
- Axum server implementation
- CLI argument parsing
- official web dashboard code
- secret logging

Used by `zz-drop` (root) and `zz-drop-tui` (`tui/`) directly. Also
consumed externally by `zz-drop-server-minimal` via git URL
(transition pending — currently still on a sibling path).

### `tui/` (zz-drop-tui binary)

Ratatui setup / configuration / management UI. Ships the `zz-tui`
binary invoked by `zz c`.

Contains:

- setup wizard
- provider configuration
- Nextcloud Login Flow UI
- profile passphrase screen
- profile push/recover screens
- account/alias management UI
- diagnostics UI

Must NOT contain:

- duplicate crypto logic
- duplicate profile format definitions
- duplicate API DTOs
- server implementation
- CLI upload logic except via shared library calls

Depends on `zz-drop-core` (workspace member at `core/`).

## Mandatory project-wide rule

Do **not** make isolated changes.

Every change must consider:

- CLI behavior (commands, flags, output, exit codes,
  scriptability)
- TUI behavior
- local agent protocol
- profile-blob compatibility (both `profile-local.zz` and
  `profile-remote.zz` slots)
- server API compatibility
- security/privacy/logging
- README/docs
- tests
- release/install impact
- cross-crate ripple inside this workspace

Public surfaces frozen at v1 (G1):

- command grammar
- exit code table (`pub const EXIT_*`)
- `--json` NDJSON event schema (`v:"1"`) when scriptable mode
  ships — see `docs/scriptable.md` once landed
- `profile.zz` envelope and payload schema
- agent protocol semantics

After freeze, additive changes are allowed; renames or removals
are breaking and need a major bump.

A change is incomplete if it changes behavior without updating
the relevant docs/spec/tests in the same patch.

## Security rules

Never log:

- provider credentials
- app passwords
- OAuth tokens
- profile decrypt passphrases
- decrypted profile data
- plaintext profile payload (decoded `profile-local.zz` /
  `profile-remote.zz` content)
- Authorization headers
- session tokens
- persistent IP/user-agent logs associated with profiles

The server must never see decrypted profile data or provider
metadata.

The local agent must:

- bind a per-user Unix socket
- enforce a 0600 token file
- check peer UID
- keep decrypted profile only in RAM
- enforce TTL and idle locked-exit

## Documentation rule

If behavior changes, update docs in the same change.

- command changes → update command docs and README
- security behavior changes → update `SECURITY.md` or the
  relevant threat-model documentation
- agent protocol changes → update protocol docs
- profile format changes → update profile format docs and
  migration notes

## Definition of done

A change is not complete unless:

- code builds (workspace-wide)
- tests pass or are updated
- docs are updated if needed
- security impact has been considered
- backward compatibility has been considered
- no scope creep was introduced
