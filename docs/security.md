# Security

This page describes what `zz-drop` does to protect your data, what it
**does not** try to protect against, and how to report a vulnerability.
The wording is deliberately conservative: we describe the design,
not absolutes.

## Threat model in one sentence

The cloud provider (Nextcloud over WebDAV, Google Drive over OAuth,
others as they land) and any zz-drop API server (`zz-drop.net` or
self-hosted, gated behind the `remote` Cargo feature in v1) **must
not** be able to read the decrypted container, the container
passphrase, or the per-provider credentials inside. The local
user's account on their own machine is trusted: zz-drop does not
defend against another process running under your UID.

## What the server sees

When the `remote` feature is enabled and the operator pushes the
container to a `zz-drop.net`-compatible API server:

- the **email** registered with
- the **Argon2id hash** of the account password (no plaintext)
- one or more **alias** strings the operator chose
- the **encrypted container blob** — opaque ciphertext + salt +
  AEAD nonce
- timestamps and a monotonic version per blob, used for
  optimistic-concurrency `expected_version` updates
- (if 2FA is on) the **Argon2id-hashed recovery codes** and an
  **encrypted-at-rest TOTP shared seed**
- whatever HTTP-level metadata the operator (or the hosted server)
  legally must keep in their reverse proxy / abuse logs

In a default v1 build the `remote` feature is off and zz-drop never
contacts a `zz-drop.net`-compatible server: there is no API client
in the binary, no DNS resolution toward the host, and the
`zz-drop.net` string is not statically referenced.

## What the server does not see

- the **container passphrase** — derived locally, used to decrypt
  `profiles-{local,remote}.zz` only on the device, never sent on
  the wire
- the **decrypted inner profiles** — including:
  - each provider's URL or OAuth endpoints
  - the cloud account username / email
  - app passwords, OAuth access / refresh tokens
  - the chosen remote folder
  - the collision policy
  - (v1.1) the Data Master Key used for file content encryption
- any **file content** transferred through `zz s` / `zz d` —
  zz-drop talks to the cloud provider directly, the API server is
  not on that path
- the **TOTP code** typed at login — verified server-side against
  the encrypted shared seed; the seed itself is never sent to the
  client

## What zz-drop does locally

- The `profiles-{local,remote}.zz` containers are encrypted with
  **Argon2id-derived KEK + XChaCha20-Poly1305 AEAD**. Salt is 16
  random bytes per envelope; the AEAD nonce is 24 random bytes per
  encrypt — re-encrypting the container after appending an inner
  profile or refreshing an OAuth token always uses a fresh nonce.
- The KEK is held by the agent in `Zeroizing<[u8; 32]>` and zeroed
  on lock / TTL expiry / process exit.
- The decrypted container lives only in the **local agent's RAM**,
  behind a `Mutex`, with a TTL of 10 minutes (auto-lock) and an
  idle-exit of 5 minutes (process terminates).
- The agent uses a **per-user Unix domain socket** with peer-UID
  check **before** the token frame (`SO_PEERCRED` on Linux,
  `LOCAL_PEERCRED` on macOS) and a 32-byte token in a `0600` file,
  compared with `subtle::ConstantTimeEq`.
- `config.toml` is plain TOML and **never** contains secrets.
- There is **no log file**. The agent never opens stdout/stderr.

## What the daily commands write to disk

| Command | What it writes |
|---|---|
| `zz <file>` | the file goes to the cloud provider, no local copy |
| `zz d <file>` | the file is downloaded to the current directory |
| `zz z` | the agent socket and a 32-byte token (`0600`); writes the cached "default alias" sidecar (`last-default-{local,remote}`) when picked |
| `zz q` | nothing |
| `zz w` | nothing — only deletes (`profiles-local.zz`, `profiles-remote.zz`, the sidecars, `config.toml`, the runtime dir) |

No telemetry, no analytics, no usage pings.

## TOTP 2FA (optional, server-side only)

If your account has TOTP enabled on a server that supports it, the
login flow becomes two-step:

1. `POST /auth/login` — password verified; server returns a
   short-lived `totp_required` challenge (5 min TTL).
2. `POST /auth/totp/login` — challenge + 6-digit code (or one
   recovery code) exchanged for a session token.

A few invariants:

- The TOTP shared seed is generated server-side, encrypted at rest
  with a server-side master key, and is shown to the user **once**
  at enrollment as an `otpauth://` URI / QR.
- 10 single-use recovery codes are issued at enrollment, displayed
  once, stored only as Argon2id hashes.
- TOTP **never** decrypts `profile.zz`. The local profile-decrypt
  passphrase is the only secret that protects the encrypted blob,
  and it never leaves your device.

Enrollment, recovery-code rotation, and disable live in the web
dashboard. The CLI/TUI only consume TOTP at login.

## Honest non-goals

zz-drop does **not** try to:

- hide the **filenames** or directory structure from your cloud
  provider (`zz s readme.md` puts a file called `readme.md` in the
  configured remote folder; the provider sees that)
- hide your **traffic timing or volume** from the cloud provider or
  any network observer
- defend against **another process running under your UID** —
  including a malicious shell extension, a hostile language server,
  or a copy of zz-drop running with the same agent socket
- forensically erase data from your disk or your cloud account
- guarantee **"zero IP capture"** — the cloud provider, the API
  server's reverse proxy, and your ISP all see the IP that
  initiated each request. The official server commits to **no
  persistent application-level IP/user-agent logs associated with
  profiles**, but TLS and TCP still happen on the wire.

If any of these are dealbreakers for your threat model, zz-drop is
not the right tool.

## Reporting a vulnerability

Please do **not** open a public GitHub issue for security bugs.

Use **GitHub Security Advisories** on this repository's "Security"
tab to file a private report. We acknowledge within 7 days and aim
to publish a fix or workaround within 30 days for high-severity
issues. We will credit you in the release notes unless you ask
otherwise.

For non-vulnerability questions about the security model, regular
GitHub issues are fine.

## Cross-references

- [`docs/agent.md`](agent.md) — local agent details, lifecycle,
  socket auth.
- [`docs/commands.md`](commands.md) — full command grammar +
  exit codes.
- [`../zz-drop-core/docs/profile-format.md`](../../zz-drop-core/docs/profile-format.md)
  — `profile.zz` envelope spec.
- [`../zz-drop-core/docs/security-model.md`](../../zz-drop-core/docs/security-model.md)
  — project-wide security model.
