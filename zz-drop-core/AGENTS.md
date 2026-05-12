# AGENTS.md — zz-drop-core

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

`zz-drop-core` is the **shared Rust crate** providing stable common
logic across the project.

It contains:

- profile data types
- encrypted profile envelope format (used by both
  `profile-local.zz` and `profile-remote.zz` slots)
- encrypt/decrypt profile functions
- Argon2id parameter model
- XChaCha20-Poly1305 profile crypto
- local agent protocol types
- public API DTOs
- shared error types
- provider model types
- command-independent config/path models

It must **not** contain:

- Ratatui UI
- HTTP server implementation
- CLI argument parsing
- web dashboard code
- secret logging
- provider upload implementation if it becomes too coupled

It is consumed by the CLI binary, the TUI binary, and any reference or
hosted server implementation. Breaking changes here ripple to every
consumer and require coordinated updates.

## Mandatory project-wide rule

Do **not** make isolated changes.

Every change must consider:

- CLI behavior
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

## OAuth client identifiers — single source of truth

Every public OAuth `client_id` (and the rare embedded `client_secret`,
for installed-app clients that include one) lives in
**`src/providers/oauth_clients.rs`**. Provider modules under
`src/providers/<name>/` *re-export* the constant from there; they
must never define their own `pub const FOO_CLIENT_ID`.

When adding a new OAuth-driven provider:

1. Add the new identifier(s) to `oauth_clients.rs`, wrapped in
   `match option_env!("ZZ_DROP_<PROVIDER>_CLIENT_ID")` so a forker
   can override at `cargo build` time without editing source.
2. Re-export the constant from the provider module:
   `pub use crate::providers::oauth_clients::<PROVIDER>_CLIENT_ID;`.
3. Update the override-variable table in the file header *and* in
   `zz-drop/docs/build.md` (the user-facing companion).
4. Add a non-empty / shape sanity test mirroring the existing ones
   so a missing or malformed override fails CI loudly.

This single-file rule is intentional: it lets reviewers grep one
file, lets forks override every value via build-time env vars, and
prevents accidental drift between provider modules.

## Documentation rule

If behavior changes, update docs in the same change.

- API changes → update OpenAPI
- command changes → update command docs and README
- security behavior changes → update `SECURITY.md` or the relevant
  threat-model documentation
- profile format changes → update profile format docs and migration notes
- agent protocol changes → update protocol docs

## Definition of done

A change is not complete unless:

- code builds
- tests pass or are updated
- docs are updated if needed
- API docs are updated if needed
- security impact has been considered
- backward compatibility has been considered
- no scope creep was introduced
