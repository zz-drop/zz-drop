# Cargo feature flags

Inventory of the optional Cargo features used across the zz-drop
crates, plus the lifecycle each one is on. Treat this file as the
single source of truth: when a feature graduates (default-off →
default-on → removed), update the row here in the same commit.

## `remote`

| Crate            | Default | Currently |
|------------------|---------|-----------|
| `zz-drop-core`   | off     | off       |
| `zz-drop`        | off     | off       |
| `zz-drop-tui`    | off     | off       |

**What it gates.** Everything that talks to `zz-drop.net`:

- `zz-drop-tui::api_client` (the HTTP module)
- the `account_email_input` / `push_alias_input` request handlers
  in `tui/src/main.rs`
- the REMOTE block in the welcome menu (`OpenRemote`,
  `ConfigureRemote`, `SignIn`)
- the default value of `App::api_base` (the `https://zz-drop.net`
  string is only embedded when the feature is on)

**What stays compiled regardless.** The DTO types in
`zz-drop-core::api` — `zz-drop-server-minimal` and tooling depend
on them directly. Gating only happens on the *client-side* code
that hits the wire.

**Why it exists.** v1 ships local-only by design. The remote
surface (account, push/pull, sign-in to recover containers across
machines) graduates in v2. The flag lets the v1 binary be built
without any zz-drop.net code path at all — useful for security
auditing the default binary and for keeping CI honest about what
"local-only" means.

**Lifecycle.**

1. v1 release: `default = []`, `remote = []`. Default builds carry
   no remote code.
2. v2 ramp: a couple of cycles with `default = ["remote"]` so the
   path becomes the norm while the flag is still removable.
3. v2 stable: feature flag and `cfg` annotations get deleted in a
   single mechanical PR. No code change otherwise.

Every default flip needs a `DECISION_LOG.md` entry citing this
file (see "v1 = local, v2 = remote" entry from 2026-05-02).

**How to verify the default binary stays clean.**

```sh
cargo build --release
strings target/release/zz-tui | grep -iE 'zz-drop\.net|account_email|push_blob'
# expected: no output
```

When the feature is on, the same grep returns the expected
references (`https://zz-drop.net`, `account_email_input`, etc.).

**How to opt in for testing.**

```sh
cargo build --features remote
cargo test --features remote
```

CI is expected to run `cargo test` *and* `cargo test --features
remote` on every commit so neither config drifts.
