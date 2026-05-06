# Nextcloud / WebDAV provider

zz-drop's first-class provider in v1 is Nextcloud, accessed over its
WebDAV endpoint at `/remote.php/dav/files/<user>/`.

## Status

The library `NextcloudClient` exposes `upload`, `download`,
`list`, `ensure_remote_root`. The CLI verbs `zz <file>` /
`zz d <file>` use it via the `RemoteFs` abstraction.

## What it does

- HTTP/1.1 over TLS via `ureq` 2.x (rustls). TLS verification is on
  by default and there is no flag to turn it off.
- Basic-auth header with an **app password** (Nextcloud setting →
  Security → "Devices & sessions"). The plaintext app password lives
  only inside the encrypted `profile.zz` and the in-RAM agent.
- `MKCOL` creates the remote root if missing. `MKCOL` returning `405`
  ("already exists") is treated as success.
- `PUT` for upload. `GET` for download. `HEAD` to detect collisions.
- `PROPFIND` with `Depth: 1` is available on the library for
  remote-listing (used by `da` / `dar` to enumerate). The
  reference parser walks the multistatus response and skips
  the first response element (the queried collection itself).

## Authentication

| Method | Status |
|---|---|
| App password (basic auth) | implemented |
| Login Flow | TASK 13 |
| Plain password | not implemented |

The app password is preferred over the user's login password and is
the only path supported in v1's API surface that this milestone
exposes.

## Path handling and security

Every filename is validated before being sent on the wire:

| Rule | Behavior |
|---|---|
| empty | rejected |
| `.` or `..` | rejected (no traversal) |
| contains `/` or `\` | rejected (no traversal) |
| contains NUL byte | rejected |

Each segment is percent-encoded (`encode_segment`) before being
joined into a URL path. The encoder treats unicode bytes UTF-8,
escapes ASCII control characters and a conservative set of reserved
characters (`%`, `?`, `#`, space, etc.).

Collision policy values come from `zz_drop_core::CollisionPolicy`:

| Policy | Behavior |
|---|---|
| `Rename` | try `name.ext`, `name (1).ext`, `name (2).ext`, …, up to 100 attempts |
| `Overwrite` | `PUT` directly, replacing the remote file |
| `Fail` | `HEAD` first, return `CollisionExists` if the remote exists |

The renamed-with-suffix algorithm follows Rust's `file_stem` /
`extension` convention: `foo.tar.gz` becomes `foo.tar (1).gz`,
`.bashrc` is treated as having no extension and becomes
`.bashrc (1)`.

## Errors and exit codes

`NextcloudError` is mapped to a single CLI exit code:

| Exit | Meaning |
|---|---|
| `9` | provider error (any of the variants below) |

The diagnostic message printed on stderr is short and never includes
the URL with embedded credentials, the basic-auth header content, or
the raw transport error text. `NextcloudError::diagnose(e)` returns
one of:

```
invalid server url
invalid remote path
auth failed
not found
conflict
server error
network error
unexpected response
io error
bad server response
local file error
file already exists
too many name conflicts
auth method not supported yet
```

Server statuses are bucketed into `Unauthorized` (`401` / `403`),
`NotFound` (`404`), `Conflict` (`409`), `ServerError` (`5xx`) and
`UnexpectedStatus` for anything else.

## What is intentionally NOT implemented yet

- chunked / resumable upload (out of scope per the v1 product spec)
- OAuth 2 token flow
- WebDAV `MOVE` / `COPY`
- file listing recursion (TASK 10 picks one)
- Login Flow (TASK 13)
- mock-server integration tests (TASK 27)

## Implementation entry points

- `zz_drop::providers::nextcloud::NextcloudClient`
- `zz_drop::providers::nextcloud::path::{validate_filename, encode_segment, encode_path, encode_remote_root}`
- `zz_drop::providers::nextcloud::collision::rename_with_suffix`
- `zz_drop::providers::nextcloud::webdav::{WebDavClient, BasicAuth, WebDavError}`
- `zz_drop::providers::nextcloud::parse_propfind_multistatus`

The bare WebDAV client is a thin wrapper around `ureq`: it can be
reused if a second WebDAV provider (e.g. a generic webdav target) is
ever added without tying it to Nextcloud's URL conventions.
