# Dropbox provider

zz-drop's fourth supported destination, after Nextcloud / WebDAV,
Google Drive, and OneDrive. Talks to Dropbox API v2 against any
personal or paid Dropbox account. The on-disk and on-the-wire
shapes documented here are stable; the implementation lives under
`zz-drop-core/src/providers/dropbox/`.

## What ships

- A typed [`DropboxProfile`] persisted only inside the encrypted
  `profile.zz` payload.
- An OAuth 2.0 **Authorization Code + PKCE** client without a
  `redirect_uri` (paste-code / out-of-band variant), registered in
  the Dropbox App Console as an App-folder app. No localhost
  callback, no embedded webview, no app secret in the binary.
- A REST client over `api.dropboxapi.com/2` and
  `content.dropboxapi.com/2` that covers ensure-folder, single-shot
  upload, download, list, delete (move-to-trash).
- Transparent access-token refresh: the operational client
  refreshes proactively when the token has 60 s or less left,
  exactly like the OneDrive sibling.

## OAuth scope

Four delegated scopes are wired into the Dropbox App Console:

```
files.content.write
files.content.read
files.metadata.read
account_info.read
```

- `files.content.{write,read}` — upload, download.
- `files.metadata.read` — list folder contents, get metadata.
- `account_info.read` — fetch the account email at setup so the
  profile summary can display "you upload as alice@example.com".
  Analogous to OneDrive's `User.Read`.

The Dropbox authorize URL does **not** carry a `scope` parameter
for our case — Dropbox derives the granted scopes from the app
registration. Restricting them in the URL is possible but not
useful when the registration is already minimal.

## App folder access

The app is registered as **App folder** type. The Dropbox API
operates on paths *relative to the app's sandbox*, e.g. the
`/files/upload` endpoint with path `/notes.md` writes to the
user-visible location `Apps/zz-drop/notes.md`. zz-drop never sees,
lists, or touches anything outside the app folder.

This is the smallest possible blast radius for a desktop OAuth
client: a leaked or revoked token only exposes the app folder, not
the user's whole Dropbox.

Because the App-folder sandbox is already a dedicated directory
(`Apps/zz-drop/`), the Dropbox profile's default `root_folder` is
the **empty string**. We don't add a redundant nested `zz-drop/`
underneath — that would surface user files as
`Apps/zz-drop/zz-drop/notes.md`, which reads like a typo.
Profiles persisted before this default flipped (with
`root_folder = "zz-drop"`) keep working unchanged: the path build
emits `/zz-drop/notes.md` and the user keeps seeing the legacy
nested layout. New profiles emit `/notes.md` directly.

## Setup flow

1. The TUI builds the authorize URL locally — no network
   round-trip. PKCE is mandatory: the client generates a 32-byte
   `code_verifier` from the OS RNG, derives
   `code_challenge = base64url(sha256(verifier))`, and embeds the
   challenge in the authorize URL.
2. The authorize URL is shown to the operator (with a QR encoding
   the same URL) so they can open it on any device with a
   browser.
3. The user approves on Dropbox's consent page. Because the URL
   omits `redirect_uri`, Dropbox displays the authorization code
   directly on the dropbox.com page for the user to copy back.
   This is the documented "out-of-band" variant of the
   authorization code flow.
4. The operator pastes (or types) the code back into the TUI.
5. The TUI POSTs `{code, code_verifier, grant_type=authorization_code,
   client_id}` to `https://www.dropbox.com/oauth2/token`. Dropbox
   returns an `access_token`, a `refresh_token` (because the
   authorize URL included `token_access_type=offline`), and an
   `expires_in` window.
6. The TUI fetches the user's email via
   `POST /2/users/get_current_account` so the profile summary can
   display it.

The whole exchange completes inside the TUI; the only secret typed
on the headless machine is the profile passphrase, never an OAuth
code or token.

## Endpoints

Verified against the official Dropbox OAuth guide on 2026-05-09:

- Authorization: `https://www.dropbox.com/oauth2/authorize`
  with `client_id`, `response_type=code`, `code_challenge`,
  `code_challenge_method=S256`, `token_access_type=offline`.
- Token exchange / refresh:
  `https://api.dropboxapi.com/oauth2/token` with `code`,
  `code_verifier`, `grant_type=authorization_code`, `client_id`
  for the initial exchange; `refresh_token`,
  `grant_type=refresh_token`, `client_id` for refresh. (The
  authorize endpoint lives on `www.dropbox.com`, the token
  endpoint on `api.dropboxapi.com` — POSTing the token form to
  the user-facing host returns a 400 with an HTML body.)

## Folder model

The user-visible folder for a fresh profile is `Apps/zz-drop/`
itself — `profile.root_folder` defaults to the empty string and
zz-drop writes directly into the App-folder sandbox. Sub-paths are
mirrored as nested folders underneath.

A profile may set `root_folder` to a non-empty value (the legacy
default was the literal `"zz-drop"`); in that case zz-drop creates
that one extra subfolder and the user sees content under
`Apps/zz-drop/<root_folder>/...`. The empty-default case skips the
intermediate `create_folder_v2` call entirely — the App folder is
already created automatically by Dropbox at consent time.

Path-addressing is direct: with an empty `root_folder` every
`/files/*` endpoint takes a `path` field of the form `/<…>`; with
a non-empty `root_folder` the prefix is `/<root_folder>/<…>`.
Idempotent folder creation uses `POST /2/files/create_folder_v2`;
a 409 with `path/conflict` is treated as success.

## Upload format

For files **≤ 150 MiB** zz-drop sends a single
`POST /2/files/upload` request to `content.dropboxapi.com` with
the file bytes as the body, an `application/octet-stream`
content-type, and a `Dropbox-API-Arg` header carrying the JSON
arguments (`path`, `mode`, `autorename`, `mute`,
`strict_conflict`).

For files **> 150 MiB** the API requires an `upload_session`
flow (`/files/upload_session/start` + `/append_v2` + `/finish`).
Resumable upload sessions are intentionally **out of scope for
v1** — the `upload_to` path returns
[`DropboxError::ServerError { status: 413 }`] above the cap so the
operator gets a clear signal, mirroring the OneDrive 4-MiB cap.

`Dropbox-API-Arg` values must be US-ASCII, so non-ASCII characters
in paths are escaped as `\uXXXX` (with surrogate pairs above
U+FFFF) by the helper `ascii_safe_json` in `rest.rs`.

## Download format

`POST /2/files/download` to `content.dropboxapi.com`, with the
target `path` in the `Dropbox-API-Arg` header and no body. The
response body is the raw file bytes, written atomically to the
destination path. File metadata is returned in a
`Dropbox-API-Result` response header (currently unused by zz-drop
since the file size is implicit in the body length).

## Listing

`POST /2/files/list_folder` paginates via `cursor` + `has_more`.
zz-drop loops `/files/list_folder/continue` until `has_more` is
false. Each entry's discriminator is the JSON `.tag` field, with
`folder` / `file` mapped to a [`RemoteEntry`]; `deleted` entries
are filtered out at the API level (`include_deleted=false`).

## Delete semantics

`POST /2/files/delete_v2` performs a soft delete; the file lands
in the user's trash and is automatically purged after ~30 days.

Permanent delete on personal / Plus accounts is not exposed by
this API surface, so the `hard` parameter on `delete_at` is
silently treated the same as the soft path. This is the same
defensive contract OneDrive's operational client uses.

## Error mapping

| HTTP | Body discriminator   | `DropboxError`           |
| ---- | -------------------- | ------------------------ |
| 401  |                      | `Unauthorized`           |
| 403  |                      | `Unauthorized`           |
| 409  | `path/not_found`     | `NotFound`               |
| 409  | other                | `Conflict`               |
| 429  |                      | `RateLimited`            |
| 5xx  |                      | `ServerError { status }` |

Dropbox does not return raw 404s for missing paths the way OneDrive
or Google Drive do; instead, `/files/get_metadata` and friends
return 409 with a structured error body whose `.tag` is
`path/not_found`. The classifier inspects the body to disambiguate
not-found from a generic conflict.

OAuth errors during refresh that imply revoked consent
(`invalid_grant`, `invalid_client`, `access_denied`,
`expired_token`) remap to `TokenExpired`, which the CLI surfaces
as `token expired` on stderr.

## Security posture

- `access_token` and `refresh_token` are persisted exclusively
  inside the encrypted `profile.zz` payload. They never touch disk
  in clear and never appear in logs.
- The `Debug` impls on `DropboxProfile` and `DropboxAuth` fully
  redact their contents.
- The PKCE `code_verifier` is short-lived and held in TUI state
  only for the duration of the AwaitingPaste → Exchanging window;
  it is cleared on `apply_dropbox_tokens`.
- TLS verification is on by default through `ureq`'s defaults; no
  flag bypasses it.
- The Dropbox App key is embedded in the binary. Dropbox's
  guidance for public clients treats it as published metadata; the
  App key only identifies the zz-drop application to Dropbox and
  does not grant access to user data on its own. There is no app
  secret in the binary: PKCE replaces it for public clients.
- The app is registered as App-folder type, which structurally
  prevents zz-drop from listing or touching anything outside
  `Apps/zz-drop/`.

## Backward compatibility

The `ProviderProfile` enum gains a `Dropbox` variant alongside
`Nextcloud`, `GoogleDrive` and `OneDrive`. A profile blob written
by a newer client and opened by an older one fails deserialisation
with a clear error — by design, since the older client cannot
honour the new provider.
