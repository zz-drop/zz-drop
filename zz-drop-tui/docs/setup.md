# zz-tui setup wizard

This document describes the configuration flow that `zz-tui` walks the
user through.

## Flow at a glance

```
welcome → provider → nextcloud server → nextcloud auth →
remote folder → collision policy → test upload → (TASK 14: passphrase)
```

Each step writes into a single `WizardState` in memory. The final
encrypted `profile.zz` is produced in TASK 14 (profile passphrase
screen) — this milestone keeps the profile in RAM only.

## 1. Welcome

Intro and a list of upcoming steps. `Enter` advances.

## 2. Provider

Lists the active providers (Nextcloud, Google Drive, OneDrive,
Dropbox). Proton Drive is visibly marked "planned" and is not
selectable. `Enter` continues with the focused provider.

## 3. Nextcloud — server

Single text field for the server URL. Validates with `url::Url::parse`
and accepts only `http`/`https`. Examples shown:

- `https://cloud.example.org`
- `http://localhost:8080` (for local testing)

`Enter` advances when the URL parses.

## 4. Nextcloud — auth

Three fields: method selector, username, secret.

- **Method:** `app password` (default) or `login flow` (planned, TASK 13).
  Toggle with `←` / `→` / `Space`.
- **Username:** plain text input.
- **App password / secret:** **masked** input (rendered as `*`).
  The secret stays in `WizardState` only.

Tab cycles focus between the three fields. `Enter` advances when both
username and secret are non-empty (and the method is `app password` —
selecting `login flow` is allowed but blocks the advance).

## 5. Remote folder

Single text field for the remote root, e.g. `/zz-drop`. Validated
against `zz_drop_core::providers::nextcloud::path::encode_remote_root`
— path traversal (`..`) and bad segments are refused.

If the folder does not exist on the server, zz-drop creates it on
first use (`MKCOL` idempotent).

## 6. Collision policy

Three-way selector:

| Choice | Behavior |
|---|---|
| `rename` | first upload keeps name; subsequent collisions get `(1)`, `(2)`, … |
| `overwrite` | replace remote file silently |
| `fail` | refuse to overwrite |

`↑` / `↓` cycle, `Enter` confirms.

## 7. Test upload

Press `t` to run a small synchronous test:

1. ensure remote root exists (`MKCOL` idempotent)
2. create a tiny local temp file
3. `PUT` it as `<remote_root>/.zz-drop-test-<nonce>.txt`
4. clean up the local temp
5. report `✓ upload OK` or `✗ failed: <reason>`

The TUI is paused (~1–5 s) during the network round-trip; pressing
keys is ignored until the test returns. The remote test file is left
in place — it has a deterministic prefix (`.zz-drop-test-`) so it is
easy to spot and remove. Cleanup at server side will be wired with
the rest of the integration tests in a future milestone.

The actual HTTP/WebDAV work is delegated to
`zz_drop_core::providers::nextcloud::NextcloudClient`, the same
client used by the CLI. No HTTP code lives in this crate.

## Output of the wizard

At the end of TASK 12, `WizardState` is fully populated but the
profile is **not** encrypted or persisted. TASK 14 will add:

- profile passphrase entry
- Argon2id + XChaCha20-Poly1305 envelope
- write to `~/.config/zz-drop/profile.zz`

## Keybindings (summary)

| Where | Keys |
|---|---|
| Anywhere | `Ctrl+C` quit |
| Welcome | `Enter` next, `q`/`Esc` quit |
| Provider | `Enter` next, `b`/`Esc`/`←` back, `q` quit |
| Server / Folder | type the URL/path, `Enter` next when valid, `Esc` back |
| Auth | `Tab` cycle field, `←`/`→` toggle method, `Enter` next, `Esc` back |
| Collision | `↑`/`↓` pick, `Enter` next, `b`/`Esc`/`←` back |
| Test upload | `t` run, `b`/`Esc`/`←` back, `q` quit |

In input fields (`server`, `username`, `secret`, `folder`) the `q`
character is typed normally — quitting requires `Ctrl+C`.

## Security

- The app password / auth secret never appears unmasked on screen.
- `WizardState`'s `Debug` impl prints `WizardState { <redacted> }` —
  the secret cannot leak via `{:?}` accidentally (asserted by a unit
  test).
- TLS verification on the Nextcloud HTTPS request is on by default
  (rustls + webpki-roots). No flag to disable in v1.
- The wizard does not write anything to disk in TASK 12 — the
  encrypted profile is produced and saved only in TASK 14.
