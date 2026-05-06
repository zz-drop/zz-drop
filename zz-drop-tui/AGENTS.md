# AGENTS.md — zz-drop-tui

This repository is part of the **zz-drop** project.

zz-drop is a minimalist CLI-first tool to quickly put files into — and
get files from — a configured safe cloud destination.

```bash
zz file.md      # upload
zz d file.md    # download
```

The project is not a sync tool, not a mount tool, and not a generic
cloud file manager.

## Required reading before editing

Before modifying anything in this repository, read:

- `README.md` if present
- `SECURITY.md` if present
- `CONTRIBUTING.md` if present
- relevant files under `docs/`
- this `AGENTS.md`

If the maintainer has provided additional project context for the
session, follow it. If unsure, stop and ask the maintainer before
editing — do not proceed on partial context.

## This repository's role

This repository ships the **Ratatui-based setup/configuration UI**.
It is for setup, configuration and repair — not for daily use. Daily
use is the CLI.

It contains:

- setup wizard
- provider configuration
- Nextcloud Login Flow UI (headless-friendly)
- profile passphrase screen
- profile push/recover screens
- account/alias management UI
- diagnostics UI
- design-derived TUI screens

It must **not** contain:

- duplicate crypto logic
- duplicate profile format definitions
- duplicate API DTOs
- server implementation
- CLI upload logic except via shared library / API calls

Shared types, crypto, profile envelope and agent protocol live in the
shared crate, not here.

## Mandatory project-wide rule

Do **not** make isolated changes.

Every change must consider:

- CLI behavior
- TUI behavior (terminal compatibility, low-color/plain fallback,
  100x30 target, no overflow, headless-friendly flows)
- local agent protocol
- profile-blob compatibility (both `profile-local.zz` and
  `profile-remote.zz` slots)
- server API compatibility
- security/privacy/logging
- README/docs/screenshots
- tests
- release/install impact
- cross-repository duplication

A change is incomplete if it changes behavior without updating the
relevant docs/spec/tests in the same patch.

## Security rules

Never log or display on screen:

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

The server must never see decrypted profile data or provider metadata.

The TUI must not assume a graphical desktop is available: Login Flow
shows URL/copy/QR/poll, never opens a browser by default.

## Documentation rule

If behavior changes, update docs in the same change.

- screen flow changes → update TUI docs, screen list and screenshots
- security behavior changes → update `SECURITY.md` or the relevant
  threat-model documentation
- profile format / agent protocol / API changes consumed here → update
  the corresponding docs in the shared crate, not by duplicating them

## Definition of done

A change is not complete unless:

- code builds
- tests pass or are updated
- docs are updated if needed
- screenshots are updated if visible UI changed
- security impact has been considered
- backward compatibility has been considered
- no scope creep was introduced
