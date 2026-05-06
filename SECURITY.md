# Security policy — zz-drop

## Reporting a vulnerability

Please use **GitHub Security Advisories** (the "Security" tab of
this repository) to file a private vulnerability report. Do not
open public issues, pull requests, or discussion threads for
security problems, and please don't disclose details on social
media or chat platforms before the maintainers have responded.

Acknowledgement target: **7 days**. Fix-or-workaround target for
high-severity issues: **30 days**. Reporters are credited in the
release notes unless they ask otherwise.

For non-vulnerability questions about the security model, regular
GitHub issues are fine.

## What this repository ships

`zz-drop` is the **CLI binary + local agent** in the same
executable. It does not run an HTTP server. v1 connects only to
the configured cloud provider (Nextcloud via WebDAV, Google Drive
via OAuth 2.0 device flow); the API path to a
`zz-drop.net`-compatible server is gated behind a Cargo feature
`remote`, default-off in v1 (graduates default-on in v2 — see
`zz-drop-core/docs/feature-flags.md`).

This repository's contribution to the project's security posture:

- Argon2id-derived KEK + XChaCha20-Poly1305 AEAD for the
  `profiles-{local,remote}.zz` containers (the encryption
  pipeline lives in `zz-drop-core`).
- KEK held by the agent in `Zeroizing<[u8; 32]>`; re-encrypts the
  container with a fresh nonce on every write (inner-profile
  append, OAuth token refresh) so KEK + nonce reuse is impossible
  by construction.
- per-user Unix domain socket with peer-UID check **before** the
  token frame (`SO_PEERCRED` on Linux, `LOCAL_PEERCRED` on macOS)
  + 32-byte token in a `0600` file, compared with
  `subtle::ConstantTimeEq`.
- 10-minute TTL auto-lock + 5-minute locked-idle exit.
- no log file; the agent never opens stdout/stderr.
- no telemetry, no analytics, no usage pings.

## What you should know before deploying

- **The user account on the local machine is trusted.** zz-drop
  does not defend against another process running under your UID.
- **`config.toml` never contains secrets.** Provider credentials
  live encrypted inside `profiles-{local,remote}.zz`. Session
  tokens live in the agent's RAM only.
- **No recovery if the container passphrase is lost** in v1. A
  v1.1 milestone introduces an opt-in BIP39 recovery key alongside
  the file-content E2EE work; the design is frozen in
  [`../zz-drop-core/docs/file-encryption.md`](../zz-drop-core/docs/file-encryption.md).
- **v1 ships local-only.** The `remote` feature is off by default;
  the default binary contains no network code that targets the
  `zz-drop.net` API and no `zz-drop.net` host string — see
  `zz-drop-core/docs/feature-flags.md` for the lifecycle.

## Pre-alpha hardening pass

A v1 hardening pass against the project's internal checklist
(`cargo audit`, secret-logging grep, agent socket / token review,
container crypto review, wire-protocol Debug redaction) ran on
**2026-05-03**. Outcome: no critical or high findings; two
RustSec advisories on unmaintained transitive dependencies
(`atomic-polyfill 1.0.3` via `postcard`, `paste 1.0.15` via
`image → rav1e`) were reviewed and accepted as known.

Reproduce locally with:

```sh
cargo audit
```

## Cross-references

- [`docs/security.md`](docs/security.md) — what the server sees /
  does not see, honest non-goals, threat model
- [`docs/agent.md`](docs/agent.md) — local agent details: lifecycle,
  socket auth, memory model
- [`../zz-drop-core/SECURITY.md`](../zz-drop-core/SECURITY.md) —
  project-wide security policy
- [`../zz-drop-core/docs/security-model.md`](../zz-drop-core/docs/security-model.md)
  — canonical security model document
