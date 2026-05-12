# zz-drop docs

Public docs for the CLI + agent repo.

Authoritative shared specs live in ``.

- [`commands.md`](commands.md) — pointer to the canonical command table
  upstream, plus the implementation status of this binary.
- [`agent.md`](agent.md) — local agent: lifecycle, security checks,
  state machine, exit codes.
- [`nextcloud.md`](nextcloud.md) — Nextcloud / WebDAV provider:
  authentication, path handling, collision policy, error mapping.

The local paths and `config.toml` schema are documented upstream in
`config.md`. The README in this repository carries a
short "Files and paths" section for quick reference.
