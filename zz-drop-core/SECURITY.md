# Security policy

## Reporting a vulnerability

Please use **GitHub Security Advisories** (the "Security" tab of
this repository) to file a private vulnerability report. Do not
open public issues, pull requests, or discussion threads for
security problems, and please don't disclose details on social
media or chat platforms before the maintainers have responded.

We acknowledge reports within **7 days** and aim to publish a fix
or workaround within **30 days** for high-severity issues.
Reporters are credited in the release notes unless they ask
otherwise.

For non-vulnerability questions about the security model, regular
GitHub issues are fine.

## Security model

The full model is documented in
[`docs/security-model.md`](docs/security-model.md). It is part of
the public, shared specification and applies to every
implementation (CLI, agent, TUI, hosted server, minimal reference
server).

Headlines:

- the server never sees decrypted profile data
- `profile.zz` is encrypted client-side with Argon2id +
  XChaCha20-Poly1305; the passphrase never leaves the device
- the local agent keeps decrypted profile data only in RAM
- secrets (passphrases, provider credentials, tokens, decrypted
  profile data, TOTP seeds, recovery codes) are never written
  to logs
- no intentional persistent application-level IP / user-agent
  logs are associated with profiles
- TOTP 2FA, when enabled on a server, protects **account login
  only** — it never decrypts `profile.zz`
- there is **no recovery** if the profile decrypt passphrase is
  lost (a future v1.1 milestone introduces an opt-in BIP39
  recovery key as a separate envelope wrap)

## Out of scope (v1)

- forensic local-deletion guarantees
- opaque encrypted filenames at the provider
- provider-side metadata hiding
- a team / organization access model
- a sync security model
- "absolute zero IP capture" — see the *Honest non-claims*
  section in `docs/security-model.md`

## Pre-alpha hardening pass

A v1 hardening review against the project's internal security
checklist (`cargo audit`, secret-logging grep, agent socket / token
audit, `profile.zz` crypto review, wire-protocol Debug redaction)
was completed on **2026-05-03**. Outcome: no critical or high
findings; two RustSec advisories on unmaintained transitive
dependencies (`atomic-polyfill 1.0.3` via `postcard`,
`paste 1.0.15` via `image → rav1e`) were reviewed and accepted as
known, neither carries a vulnerability.

Reproduce locally with:

```sh
( cd zz-drop-core && cargo audit )
( cd zz-drop && cargo audit )
( cd zz-drop-tui && cargo audit )
```

The redaction guarantees on wire types and provider auth structs
are locked in by canary tests; see `tests/agent_proto.rs`,
`tests/passphrase_screen.rs`, `tests/login_flow_ui.rs`, and the
per-provider unit tests.

## Cross-references

- [`docs/security-model.md`](docs/security-model.md) — full model
- [`docs/profile-format.md`](docs/profile-format.md) — `profile.zz`
  envelope spec
- [`docs/agent-protocol.md`](docs/agent-protocol.md) — local
  CLI ↔ agent protocol
- [`docs/api/openapi.yaml`](docs/api/openapi.yaml) — public HTTP
  API v1 contract
