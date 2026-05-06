# AGENTS.md — zz-drop

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

This repository ships the **CLI binary `zz-drop`** (with `zz` symlink
when available) and the **local per-user agent**, in the same binary.

It contains:

- command parser
- upload/download/list operations
- agent internal mode
- Unix socket client/server
- profile unlock flow
- local wipe
- Nextcloud/WebDAV operational client (until it gets split out)
- minimal launcher for the TUI binary via `zz c`

It must **not** contain:

- full TUI implementation
- web dashboard
- billing
- server DB logic
- marketing web pages

Shared types, crypto, profile envelope and agent protocol live in the
shared crate, not here.

## Mandatory project-wide rule

Do **not** make isolated changes.

Every change must consider:

- CLI behavior (commands, flags, output, exit codes, scriptability)
- TUI behavior
- local agent protocol
- profile-blob compatibility (both `profile-local.zz` and
  `profile-remote.zz` slots)
- server API compatibility
- security/privacy/logging
- README/docs
- tests
- release/install impact
- cross-repository duplication

A change is incomplete if it changes behavior without updating the
relevant docs/spec/tests in the same patch.

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

The server must never see decrypted profile data or provider metadata.

The local agent must:

- bind a per-user Unix socket
- enforce a 0600 token file
- check peer UID
- keep decrypted profile only in RAM
- enforce TTL and idle locked-exit

## Documentation rule

If behavior changes, update docs in the same change.

- command changes → update command docs and README
- security behavior changes → update `SECURITY.md` or the relevant
  threat-model documentation
- agent protocol changes → update protocol docs
- profile format changes → update profile format docs and migration notes

## Definition of done

A change is not complete unless:

- code builds
- tests pass or are updated
- docs are updated if needed
- security impact has been considered
- backward compatibility has been considered
- no scope creep was introduced
