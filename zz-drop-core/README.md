# zz-drop-core

Shared Rust crate for the [zz-drop](https://zz-drop.net) project.

zz-drop is a minimalist CLI-first tool to quickly put files into — and
get files from — a configured safe cloud destination.

```bash
zz file.md      # upload
zz d file.md    # download
```

It is not a sync tool, not a mount tool, and not a generic cloud file
manager.

## What this crate provides

`zz-drop-core` is the canonical home of types and logic shared between
the CLI / agent binary, the Ratatui setup UI, and any compatible server
implementation:

- profile data types
- encrypted profile envelope format (used by both
  `profile-local.zz` and `profile-remote.zz` slots on disk; see
  [`docs/profile-format.md`](docs/profile-format.md))
- profile encrypt / decrypt functions
- Argon2id parameter model
- XChaCha20-Poly1305 profile crypto
- local agent protocol types
- public API DTOs
- shared error types
- provider model types
- command-independent config and path models

## What this crate does not contain

- Ratatui UI
- HTTP server implementation
- CLI argument parsing
- web dashboard code
- secret logging
- provider upload implementation when it becomes too coupled to live here

## Public specifications

The following documents are the authoritative public specification of
the project. Implementations must conform to them.

- [`docs/commands.md`](docs/commands.md) — CLI command table and parser rule
- [`docs/profile-format.md`](docs/profile-format.md) — encrypted profile envelope and crypto (`profile-local.zz` / `profile-remote.zz`)
- [`docs/agent-protocol.md`](docs/agent-protocol.md) — local CLI ↔ agent protocol
- [`docs/security-model.md`](docs/security-model.md) — security goals, non-goals, what the server stores
- [`docs/api/README.md`](docs/api/README.md) — HTTP API v1 overview
- [`docs/api/openapi.yaml`](docs/api/openapi.yaml) — HTTP API v1 OpenAPI 3.1 contract
- [`src/api/`](src/api/) — Rust DTOs + error model that mirror the OpenAPI spec; the canonical wire format for both server and client implementations

## Versioning

API v1, profile envelope v1 and agent protocol v1 are intended to
be stable. Breaking changes require a coordinated v2 across consumers.

## Security

See [`SECURITY.md`](SECURITY.md) for the disclosure policy and headline
security properties. The full model is in
[`docs/security-model.md`](docs/security-model.md).

## License

To be confirmed before public release.
