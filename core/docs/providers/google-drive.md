# Google Drive provider

zz-drop's second supported destination, after Nextcloud / WebDAV.
The on-disk and on-the-wire shapes documented here are stable; the
implementation lives under `core/src/providers/google_drive/`.

## What ships

- A typed [`GoogleDriveProfile`] persisted only inside the encrypted
  `profile.zz` payload.
- An OAuth 2.0 **Device Authorization Grant** client (RFC 8628),
  registered with Google as a "TVs and Limited Input devices"
  client. No localhost callback, no embedded webview.
- A small REST client over `googleapis.com/drive/v3` that covers
  ensure-folder, upload (multipart), download, list, delete (default
  trash, opt-in hard).
- Transparent access-token refresh: the operational client refreshes
  proactively when the token has 60 s or less left.

## OAuth scope

The only scope requested is

```
https://www.googleapis.com/auth/drive.file
```

`drive.file` lets the app create, read, update and delete *only the
files the app itself created or that the user explicitly opened with
the app*. The user's other Drive content stays inaccessible to
zz-drop. This is intentional and not configurable in v1.

## Setup flow

1. The TUI starts the device flow. zz-drop POSTs to
   `https://oauth2.googleapis.com/device/code` with the embedded
   `client_id` and the scope.
2. Google returns a short `user_code` and a `verification_url` (plus
   a `verification_url_complete` containing the code, used for the
   QR).
3. The TUI displays both and polls the token endpoint at the
   server-supplied interval (default 5 s, automatically backed off
   on `slow_down`).
4. When the user approves the consent on a second device, Google
   issues an `access_token` + `refresh_token` pair plus an expiry
   in seconds.
5. zz-drop fetches the user's email via
   `GET /drive/v3/about?fields=user(emailAddress)` so the profile
   summary can show "you upload as alice@example.com" without
   asking the user to type it.

The whole exchange completes inside the TUI; the only secret typed
on the headless machine is the profile passphrase, never an OAuth
code or token.

## Folder model

zz-drop creates a single root folder under "My Drive", named after
`profile.root_folder` (default `zz-drop`). All uploads land inside
it; sub-paths are mirrored as nested folders. The `drive.file` scope
hides any folder the user might have created themselves with the
same name — zz-drop will create its own and operate on that one.

The root folder ID is cached lazily inside the profile (under
`root_folder_id`) after the first lookup, to skip a search round-trip
on subsequent runs.

## Upload format

Files are sent as `multipart/related` requests against
`https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart`.
The body has two parts:

1. `Content-Type: application/json; charset=UTF-8` with the file
   metadata (name, parents).
2. `Content-Type: application/octet-stream` with the file bytes.

The boundary is randomised per request from `OsRng`, prefixed
`zzdrop_` for log readability. zz-drop uses `application/octet-stream`
as the content-type for every file: Drive preserves the bytes
faithfully and the actual mime is not interpreted by the storage
layer in our use case.

Multipart upload is suitable for small-to-medium files. Resumable
upload (`uploadType=resumable`) is not implemented in v1; the public
API of the operational client will be source-compatible when it
lands.

## Download format

`GET /drive/v3/files/{fileId}?alt=media` returns the raw bytes,
which zz-drop writes atomically to the destination path.

## Listing

`GET /drive/v3/files?q=...&spaces=drive&pageSize=200` with the
filter

```
'<folderId>' in parents and trashed = false
```

paginates through `nextPageToken`. With `drive.file` the listing
already excludes content the app didn't create; the `trashed = false`
clause hides anything the user moved to trash.

## Delete semantics

Default: PATCH the file with `{"trashed": true}`. The file is
recoverable from the user's Drive UI for ~30 days. This matches the
"safety first" posture of zz-drop's other destructive operations.

Opt-in: `DELETE /drive/v3/files/{fileId}` removes the file
permanently. Surfaced through the `hard` parameter of `delete_at`
on the operational client; the CLI does not surface a `--purge`
flag in v1.

## Error mapping

| HTTP | Cause                                | `GoogleDriveError`            |
| ---- | ------------------------------------ | ----------------------------- |
| 401  | invalid / revoked token              | `Unauthorized`                |
| 403  | `*RateLimitExceeded` in body         | `RateLimited`                 |
| 403  | other (insufficient permissions)     | `Unauthorized`                |
| 404  | file or folder gone                  | `NotFound`                    |
| 409  | conflict on overwrite                | `Conflict`                    |
| 429  | quota exceeded                       | `RateLimited`                 |
| 5xx  | transient server error               | `ServerError { status }`      |

OAuth errors during refresh that imply the user revoked consent
(`access_denied`, `expired_token`, `invalid_client`, `invalid_grant`)
are remapped to `TokenExpired`, which the CLI surfaces as
`token expired` on stderr.

The `diagnose` helper produces a single short stderr line per error
variant, matching the Nextcloud provider's convention so scripts
parsing zz-drop output stay simple.

## Security posture

- `access_token` and `refresh_token` are persisted exclusively
  inside the encrypted `profile.zz` payload. They never touch disk
  in clear and never appear in logs.
- The `Debug` impls on `GoogleDriveProfile` and `GoogleDriveAuth`
  fully redact their contents; the `Debug` on the OAuth
  `DeviceCodeResponse` redacts the `device_code` while keeping the
  user-facing `user_code` and `verification_uri` visible.
- TLS verification is on by default through `ureq`'s defaults; no
  flag bypasses it.
- The OAuth `client_id` and `client_secret` are embedded in the
  binary. Per Google's own guidance for installed apps, the
  `client_secret` is not treated as a real secret; embedding it is
  the documented approach. The pair only identifies the zz-drop
  application to Google; it does not grant access to user data on
  its own.

## Backward compatibility

The `ProviderProfile` enum gains a `GoogleDrive` variant alongside
`Nextcloud`. A profile blob written by a newer client and opened by
an older one fails deserialisation with a clear error — by design,
since the older client cannot honour the new provider.
