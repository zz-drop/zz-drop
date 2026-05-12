# Nextcloud provider

zz-drop's first supported destination, before Google Drive and
OneDrive arrived. Talks to a Nextcloud server (self-hosted or
managed) over its WebDAV endpoint. The on-disk and on-the-wire
shapes documented here are stable; the implementation lives under
`zz-drop-core/src/providers/nextcloud/`.

## What ships

- A typed [`NextcloudProfile`] persisted only inside the encrypted
  `profile.zz` payload.
- A WebDAV client over `ureq` 2.x (rustls), with HTTP/1.1 + TLS
  verification on by default and no flag to disable it.
- Two authentication paths: classic **app password** (basic auth),
  and **Login Flow v2** for headless setup. Both produce the same
  on-the-wire credential — an app password — that the encrypted
  profile stores.
- A REST-ish surface over WebDAV verbs: `MKCOL`, `PROPFIND`, `PUT`,
  `GET`, `HEAD`, `DELETE`, with paths under
  `/remote.php/dav/files/<user>/<root>/...`.

## Authentication

zz-drop never persists the user's primary Nextcloud password.
Either authentication path resolves to an **app password** — a
per-device credential the operator can revoke from Nextcloud's
"Security → Devices & sessions" UI without touching the main
account password.

### Method 1 — app password (manual)

The operator generates an app password in the Nextcloud web UI
and pastes it into the wizard. This is the simplest path on a
shell you already use day-to-day.

### Method 2 — Login Flow v2

For headless boxes (servers, workstations the user logs into via
SSH only), zz-drop initiates the Login Flow:

1. `POST /index.php/login/v2` returns a `login` URL the user opens
   in any browser, plus a `poll.token` and a `poll.endpoint`.
2. The operator authenticates in the browser; behind the scenes
   Nextcloud associates the token with the session.
3. zz-drop polls `poll.endpoint` until Nextcloud returns
   `{ server, loginName, appPassword }`.
4. The wizard saves the credentials into the encrypted profile.

The whole exchange completes inside the TUI; the only secret typed
on the headless machine is the profile passphrase, never an
account password.

## Folder model

zz-drop creates a single root folder under the user's WebDAV root
(default `zz-drop`). All uploads land inside it; sub-paths are
mirrored as nested collections. The first upload calls `MKCOL` on
the configured root; `405` ("already exists") is treated as
success so the operation is idempotent.

There is no "files-created-by-app" filter on Nextcloud: the user
sees, can read, and can move zz-drop's files exactly like any
other content under the configured root.

## Path handling and security

Every filename / path segment is validated before being sent on
the wire:

| Rule                                  | Behavior                       |
| ------------------------------------- | ------------------------------ |
| empty                                 | rejected                       |
| `.` or `..`                           | rejected (no traversal)        |
| contains `/` or `\`                   | rejected (no traversal)        |
| contains NUL byte                     | rejected                       |

Each segment is percent-encoded (`encode_segment`) before being
joined into a URL path. The encoder treats unicode bytes as UTF-8,
escapes ASCII control characters and a conservative set of
reserved characters (`%`, `?`, `#`, space, etc.).

Collision policies (from `zz_drop_core::CollisionPolicy`):

| Policy      | Behavior                                                                |
| ----------- | ----------------------------------------------------------------------- |
| `Rename`    | try `name.ext`, `name (1).ext`, …, up to 100 attempts                   |
| `Overwrite` | `PUT` directly, replacing the remote file                               |
| `Fail`      | `HEAD` first, return `CollisionExists` if the remote already exists     |

The renamed-with-suffix algorithm follows Rust's `file_stem` /
`extension` convention: `foo.tar.gz` becomes `foo.tar (1).gz`,
`.bashrc` is treated as having no extension and becomes
`.bashrc (1)`.

## Upload format

`PUT /remote.php/dav/files/<user>/<root>/<path>` with the file
bytes as the body and a `Content-Length` header. zz-drop sets
`Content-Type: application/octet-stream` and lets Nextcloud
preserve the bytes faithfully — the actual mime type isn't
interpreted by the storage layer for our use case.

Chunked / resumable upload is intentionally not implemented in
v1; for the file sizes zz-drop targets (notes, configs, modest
archives) a single PUT covers the working range.

## Download format

`GET /remote.php/dav/files/<user>/<root>/<path>` returns the raw
bytes, which zz-drop writes atomically to the destination path.

## Listing

`PROPFIND /remote.php/dav/files/<user>/<root>/<prefix>` with
`Depth: 1` and a body requesting the standard DAV properties.
The reference parser walks the multistatus response and skips
the first response element (the queried collection itself), so
the resulting list contains only children.

## Delete semantics

`DELETE /remote.php/dav/files/<user>/<root>/<path>` performs the
delete on Nextcloud's side. The file lands in the user's
"Deleted files" view (`Trashbin`) and is recoverable from the
Nextcloud web UI within the server's configured retention
window. zz-drop does not surface a "permanent delete" option
in v1.

## Error mapping

| HTTP   | Cause                                | `NextcloudError`              |
| ------ | ------------------------------------ | ----------------------------- |
| 401    | invalid app password                 | `Unauthorized`                |
| 403    | server-side ACL / lock               | `Unauthorized`                |
| 404    | file or folder gone                  | `NotFound`                    |
| 405    | `MKCOL` on existing folder           | treated as success            |
| 409    | conflict on overwrite                | `Conflict`                    |
| 5xx    | transient server error               | `ServerError { status }`      |
| other  | unrecognised status                  | `UnexpectedStatus { status }` |

The `diagnose` helper produces a single short stderr line per
error variant, matching the convention used by the Google Drive
and OneDrive providers so scripts parsing zz-drop output stay
simple. The diagnostic never includes the URL with embedded
credentials, the basic-auth header content, or the raw transport
error text.

## Security posture

- The app password is persisted exclusively inside the encrypted
  `profile.zz` payload. It never touches disk in clear and never
  appears in logs.
- The `Debug` impls on `NextcloudProfile` and `NextcloudAuth`
  fully redact their contents.
- TLS verification is on by default through `ureq`'s defaults; no
  flag bypasses it.
- The server URL is treated as user input and validated as a URL;
  schemes other than `http://` and `https://` are rejected.
- `NextcloudAuth::AppPassword` is the only persisted authentication
  variant in v1. The Login Flow path *produces* an app password —
  it does not store the polling token in the profile.

## Backward compatibility

The `ProviderProfile::Nextcloud` variant is the original member of
the enum. Newer variants (`GoogleDrive`, `OneDrive`) sit alongside
it; a profile blob written by a newer client and opened by an
older one fails deserialisation with a clear error — by design,
since the older client cannot honour the newer provider.

## Implementation entry points

- `zz_drop_core::providers::nextcloud::NextcloudClient`
- `zz_drop_core::providers::nextcloud::path::{validate_filename, encode_segment, encode_path, encode_remote_root}`
- `zz_drop_core::providers::nextcloud::collision::rename_with_suffix`
- `zz_drop_core::providers::nextcloud::webdav::{WebDavClient, BasicAuth, WebDavError}`
- `zz_drop_core::providers::nextcloud::parse_propfind_multistatus`
- `zz_drop_core::providers::nextcloud::login_flow::LoginFlowClient`

The bare WebDAV client is a thin wrapper around `ureq`: it can be
reused if a second WebDAV provider (e.g. a generic WebDAV target)
is ever added without tying it to Nextcloud's URL conventions.
