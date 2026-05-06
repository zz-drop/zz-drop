# Security policy — zz-drop-tui

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

`zz-drop-tui` is the **setup and configuration TUI** (`zz-tui`
binary). It is not the daily interface — daily upload / download
lives in the [`zz-drop`](https://github.com/zz-drop/zz-drop) CLI.

This binary's contribution to the project's security posture:

- collects what goes into the `profiles-local.zz` container
  (per-provider URL / OAuth tokens, username, app password or
  Login Flow result, remote folder, collision policy) and writes
  the encrypted container with mode `0600`
- masks the container passphrase on screen (`*` characters); the
  plaintext is dropped from RAM as soon as the encrypt function
  returns
- shows a **zxcvbn**-based strength meter and a weak-passphrase
  confirmation prompt
- when **adding a new inner profile** to an unlocked container,
  uses the cached KEK to re-encrypt with a fresh nonce — the
  operator never re-types the passphrase, and a fresh inner
  profile cannot share a nonce with an older one
- the **Nextcloud Login Flow** is headless-friendly: a phone (or
  any second device with a browser) is enough to complete
  authentication; no browser on the box running `zz-tui` is
  required
- the **Google Drive OAuth device flow** likewise needs only a
  browser on a second device; tokens land directly inside the
  encrypted container
- the **push sub-flow** to a `zz-drop.net`-compatible API
  (gated by the `remote` Cargo feature, default-off in v1) talks
  over TLS; the bearer token lives only in `PushFlowState` for
  the duration of the run, with a custom `Debug` impl that elides
  it
- no log file; no telemetry

## What this binary does not handle

- the daily upload/download path — that's the CLI
- the Nextcloud transport — that's `zz-drop-core`
- profile encryption primitives — also `zz-drop-core`

If you find a security issue that touches one of those, please
file it against the relevant repository's `SECURITY.md`.

## Cross-references

- [`README.md`](README.md) — Status, screens, terminal requirements,
  QR rendering policy
- [`docs/screens.md`](docs/screens.md) — full screen catalogue
- [`docs/login-flow.md`](docs/login-flow.md) — Nextcloud Login Flow
  v2 specifics
- [`docs/profile-passphrase.md`](docs/profile-passphrase.md) —
  passphrase entry + strength meter + weak-warning sub-state
- [`../zz-drop/docs/security.md`](../zz-drop/docs/security.md) —
  what the server sees / does not see, honest non-goals
- [`../zz-drop-core/docs/security-model.md`](../zz-drop-core/docs/security-model.md)
  — canonical security model document
