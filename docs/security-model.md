# zz-drop security model

This document describes the security posture of zz-drop. It is part of
the public, shared specification and applies to every implementation
(CLI, agent, TUI, hosted server, minimal reference server).

## Security goals

- The server never sees decrypted profile data.
- `profile.zz` is encrypted client-side.
- The profile decrypt passphrase never leaves the device.
- Provider credentials are stored only inside encrypted `profile.zz` and
  in RAM when the local agent is unlocked.
- The local agent holds the decrypted profile only in RAM.
- No intentional persistent application IP/user-agent logs are associated
  with profiles.
- Secrets (passphrases, provider credentials, tokens, decrypted profile
  data) are never written to logs.

## Non-goals (v1)

- No forensic local-deletion guarantee.
- No opaque encrypted filename mapping at the provider.
- No provider-side metadata hiding.
- No sync security model.
- No team / organization access model.

## Two-secret password model

zz-drop has exactly two user-memorized secrets in v1:

1. **Web account password** — used to authenticate against the API
   (login, profile blob upload/download). The server stores only its
   Argon2id hash.
2. **Profile decrypt passphrase** — used locally to decrypt `profile.zz`.
   It is never sent to the server.

There is no separate alias / profile-download password in v1.

A server **may** add an optional possession factor (TOTP) on top of
the account password — see [Account 2FA](#account-2fa-server-side-optional)
below — but TOTP is a server-issued seed, not a third user-memorized
secret. The "two memorized secrets" invariant is preserved.

## Local agent

- One agent per user. Same binary as the CLI in a hidden internal mode.
- Unix domain socket, primary at `$XDG_RUNTIME_DIR/zz-drop/agent.sock`,
  fallback at `/tmp/zz-drop-$UID/agent.sock`. Directory permissions 0700.
- Token file with 0600 permissions, 32 random bytes.
- Per-connection peer UID credential check (`SO_PEERCRED` on Linux,
  `getpeereid()` on macOS/BSD).
- Decrypted `PlainProfile` lives in RAM only.
- `zz q` clears RAM immediately.
- Automatic lock after `unlock_ttl_secs = 600` (10 minutes).
- Locked-idle exit after `agent_idle_exit_secs = 300` (5 minutes).

See `agent-protocol.md` for the wire protocol.

## `profile.zz` envelope

- JSON envelope, version 1.
- Argon2id KDF; parameters stored in the envelope (`memory_kib =
  194_560 ≈ 190 MiB, iterations = 3, parallelism = 1`).
- XChaCha20-Poly1305 AEAD with a fresh 24-byte random nonce per
  encrypt; salt is 16 random bytes per envelope.
- Salt, nonce and ciphertext are base64-encoded.
- Encrypted payload format: CBOR.
- KDF target: ~500 ms – 1 s on an average machine.
- No recovery if the container passphrase is lost. (A v1.1 opt-in
  BIP39 recovery key is described in `file-encryption.md`.)

The payload is a `ProfileSet` — a container that holds N inner
profiles (one inner profile per "alias", e.g. `casa-nc`,
`gdrive-bright`). The agent caches the 32-byte KEK derived by
Argon2id in zeroizing storage and re-encrypts the container with
a fresh nonce when an inner profile is appended or an OAuth token
gets refreshed: same KEK, fresh nonce per write — no nonce reuse
is possible by construction.

See `profile-format.md` for the envelope schema.

## File content encryption (v1.1, design frozen)

v1 ships **container-level** E2EE only: the cloud provider sees
plaintext file content, the `profile.zz` is opaque ciphertext.
v1.1 extends client-side encryption to the actual user files
uploaded to providers that don't already provide trustworthy
zero-knowledge storage of their own.

The design is frozen: per-provider 32-byte Data Master Key (DMK)
generated at setup, stored only inside the encrypted payload of
`profile.zz`, optionally backed up via a BIP39 24-word recovery
key. The `.zzd` file format on the cloud is auto-detected on
download via a magic header — no `dx` / `de` modifier is needed.
File content encryption is **opt-in per provider**, default off:
the operator explicitly accepts the consent screens (loss of
provider web/app file access, no share links, no full-text
search) before encryption activates.

Full byte-level spec, threat model and recovery-key flow:
[`file-encryption.md`](file-encryption.md). v1 implementations
must be forward-compatible with the v1.1 envelope extensions
(the new fields are optional — older `profile.zz` files without
them remain valid).

## Server: what is stored

The server stores **only**:

- account email
- account password hash (Argon2id)
- profile alias
- the encrypted `profile.zz` blob (opaque to the server)
- blob size and blob version
- `created_at` / `updated_at` timestamps
- email preferences
- billing state, when applicable

## Server: what is NOT stored

The server never stores or processes:

- the profile decrypt passphrase
- decrypted profile data
- the cloud provider URL
- the cloud provider username
- provider app passwords or OAuth tokens
- the user's uploaded or downloaded files
- intentional persistent application IP / user-agent logs associated
  with profiles

## Account 2FA (server-side, optional)

Servers compatible with the API v1 contract **may** offer optional
TOTP-based two-factor authentication on the account login flow.
`zz-drop-server-minimal` implements it; the hosted service will too.
A server that opts out simply does not expose the `/auth/totp/*`
endpoints — the absence is the signal.

Scope of TOTP in this project:

- TOTP protects **server-side login** and privileged account
  operations (change password, delete account, enroll/disable 2FA,
  rotate recovery codes).
- TOTP is **never** used to decrypt `profile.zz`. The local profile
  decrypt passphrase remains the only secret that protects the
  encrypted blob, and it never leaves the device.
- TOTP is opt-in per account. Default off.
- Enrollment, recovery-code view, and disable live in the **web
  dashboard only** in v1. CLI/TUI clients only **consume** TOTP at
  login.

Server-side handling:

- TOTP shared seed: generated server-side, stored encrypted at rest
  with a server-side master key, exposed to the user once at
  enrollment via an `otpauth://` URI / QR.
- Algorithm: TOTP per RFC 6238 (HMAC-SHA1, 6 digits, 30 s period,
  drift tolerance ±1 step).
- Recovery codes: 10 single-use codes issued at enrollment, shown
  once, stored only as Argon2id hashes.
- Verification rate-limit: 5 failures per 15 minutes per account;
  ephemeral (in-memory) state only — no persistent IP capture.
- Disable: requires the account password **plus** a current valid
  TOTP code or one recovery code.
- Two-step login on the wire: `POST /auth/login` returns a
  short-lived `totp_required` challenge; `POST /auth/totp/login`
  exchanges the challenge plus a code (TOTP or recovery) for a
  session.

Why TOTP is unsuitable as a *decrypt* factor: a 6-digit, 30-second
code carries ≈ 20 bits of entropy and is non-deterministic over
time, so it cannot derive a stable AEAD key. Putting the TOTP
shared seed itself into the KDF would only add a static second
secret, not a true second factor, and would defeat the temporal
property that makes TOTP useful in the first place. A real local
second factor (hardware token, OS Secure Enclave, FIDO2 PRF) is a
separate question for a future profile envelope and is explicitly
out of scope here.

## Logging rules

The following must never appear in logs:

- account passwords
- profile decrypt passphrases
- decrypted profile data
- plaintext `profile.zz` payload
- provider credentials (URL, username, app password, OAuth token)
- session tokens
- `Authorization` headers
- TOTP shared seeds, TOTP codes typed at login, recovery codes
- persistent IP / user-agent logs associated with profiles

## Disclosure policy

To report a security issue, open a **private security advisory** on
the repository's "Security" tab. Do not file public issues, pull
requests, or discussion threads for security problems, and do not
disclose details on social media or chat platforms before the
maintainers have responded.

Each repository's `SECURITY.md` carries the same disclosure policy
and points back to this document for the full model.

## Hardening passes

A pre-alpha hardening pass against the project's internal
checklist (`cargo audit`, secret-logging grep, agent socket /
token review, container crypto review, wire-protocol Debug
redaction) ran on **2026-05-03**. Outcome: no critical or high
findings; two RustSec advisories on unmaintained transitive
dependencies (`atomic-polyfill 1.0.3` via `postcard`,
`paste 1.0.15` via `image → rav1e`) were reviewed and accepted
as known, neither carries a vulnerability. The headline lives in
each repo's `SECURITY.md`; the redaction guarantees on wire
types and provider auth structs are locked in by canary tests
under `tests/`.

## Honest non-claims

zz-drop deliberately does **not** promise:

- "zero IP logging" — TLS and TCP still happen on the wire, every
  reverse proxy and ISP sees IPs that initiate requests. The
  invariant we hold is: **no persistent application-level IP /
  user-agent logs are intentionally associated with profiles**.
  That is enforceable; "absolute zero IP" is not.
- forensic erasure of local data
- hiding filenames or directory structure from the cloud provider
- protection against another process running under the same user
  ID on the same machine
- backup, restore, monitoring, abuse handling, or legal compliance
  for self-hosted minimal-server deployments

If any of these are dealbreakers, zz-drop is not the right tool.
