# OneDrive provider

zz-drop's third supported destination, after Nextcloud / WebDAV and
Google Drive. Talks to Microsoft Graph against personal Microsoft
accounts and work / school accounts (multi-tenant). The on-disk
and on-the-wire shapes documented here are stable; the implementation
lives under `core/src/providers/onedrive/`.

## What ships

- A typed [`OneDriveProfile`] persisted only inside the encrypted
  `profile.zz` payload.
- An OAuth 2.0 **Device Authorization Grant** client (RFC 8628),
  registered in Microsoft Entra as a public-client app with "Allow
  public client flows" enabled. No localhost callback, no embedded
  webview.
- A REST client over `graph.microsoft.com/v1.0` that covers
  ensure-folder, upload (PUT for ≤ 4 MiB, upload session for
  larger), download, list, delete (default soft, opt-in hard).
- Transparent access-token refresh: the operational client refreshes
  proactively when the token has 60 s or less left, exactly like
  the Google Drive sibling.

## OAuth scope

Three delegated scopes are requested at the device endpoint:

```
Files.ReadWrite offline_access User.Read
```

- `Files.ReadWrite` — read/write access to the user's OneDrive.
- `offline_access` — required for the token endpoint to issue a
  `refresh_token`. Microsoft does not issue refresh tokens unless
  this scope is asked, even for public clients.
- `User.Read` — read `mail` / `userPrincipalName` so the profile
  summary can display "you upload as alice@example.org" without
  asking the operator to type the address.

These are all "user-consentable" scopes; the operator confirms
consent at the verification page, no admin pre-authorisation
required for personal or standard work accounts.

## Setup flow

1. The TUI starts the device flow. zz-drop POSTs to
   `https://login.microsoftonline.com/common/oauth2/v2.0/devicecode`
   with the embedded `client_id` and the scope string above.
2. Microsoft returns a short `user_code`, the
   `verification_uri` (`https://login.microsoft.com/device`), and a
   `device_code` plus an `expires_in` window.
3. The TUI displays the user-facing pair (with a QR encoding the
   URL) and polls
   `https://login.microsoftonline.com/common/oauth2/v2.0/token` at
   the server-supplied interval (default 5 s, automatically backed
   off on `slow_down`).
4. When the user approves on a second device, Microsoft issues an
   `access_token` + `refresh_token` pair.
5. zz-drop fetches the user's email via `GET /me?$select=mail,
   userPrincipalName` so the profile summary can show it.

The whole exchange completes inside the TUI; the only secret typed
on the headless machine is the profile passphrase, never an OAuth
code or token.

## Tenant choice

zz-drop targets `/common`, which accepts both personal Microsoft
accounts (`@outlook.com`, `@hotmail.com`, …) and Microsoft 365
work / school accounts. `/consumers` would lock to personal only;
`/organizations` to work only. `/common` covers the broader
audience without configuration on the operator's side.

## Folder model

zz-drop creates a single root folder under the user's OneDrive,
named after `profile.root_folder` (default `zz-drop`). All uploads
land inside it; sub-paths are mirrored as nested folders. The root
folder ID is cached lazily inside the profile (under
`root_folder_id`) after the first lookup, to skip a search
round-trip on subsequent runs.

OneDrive's drive item path uses `/me/drive/root:/<segments>` for
path-based addressing, and `/me/drive/items/<id>` for id-based.
zz-drop uses the id-based form once the root folder is resolved.

## Upload format

For files **≤ 4 MiB** zz-drop sends a single
`PUT /me/drive/items/<parent_id>:/<filename>:/content` request
with the file bytes as the body.

For files **> 4 MiB** zz-drop creates an *upload session* via
`POST /me/drive/items/<parent_id>:/<filename>:/createUploadSession`
and uploads the bytes in chunks against the returned upload URL.
Resumable upload semantics are inherited from the Graph contract:
a chunked upload survives transient network failures within the
session lifetime.

## Download format

`GET /me/drive/items/<id>/content` returns the raw bytes, which
zz-drop writes atomically to the destination path. The first
request is satisfied by an HTTP 302 to the actual storage CDN;
zz-drop follows the redirect transparently.

## Listing

`GET /me/drive/items/<folder_id>/children?$top=200` paginates
through `@odata.nextLink`. The OneDrive listing already excludes
deleted items at the API level; zz-drop does not need a "trashed"
filter.

## Delete semantics

Default: `DELETE /me/drive/items/<id>` performs a soft delete; the
file lands in the user's recycle bin and can be restored from
OneDrive's web UI for a provider-defined retention window.

Opt-in: the same DELETE with `Prefer: permanent` causes a permanent
delete. Surfaced through the `hard` parameter of `delete_at` on the
operational client; the CLI does not surface a `--purge` flag in v1.

## Error mapping

| HTTP | Cause                                | `OneDriveError`               |
| ---- | ------------------------------------ | ----------------------------- |
| 401  | invalid / revoked token              | `Unauthorized`                |
| 403  | insufficient permissions / consent   | `Unauthorized`                |
| 404  | file or folder gone                  | `NotFound`                    |
| 409  | conflict on overwrite                | `Conflict`                    |
| 429  | quota / throttling                   | `RateLimited`                 |
| 5xx  | transient server error               | `ServerError { status }`      |

OAuth errors during refresh that imply revoked consent
(`invalid_grant`, `interaction_required`, `consent_required`)
are remapped to `TokenExpired`, which the CLI surfaces as
`token expired` on stderr.

The `diagnose` helper produces a single short stderr line per
error variant, matching the convention used by the Nextcloud and
Google Drive providers so scripts parsing zz-drop output stay
simple.

## Security posture

- `access_token` and `refresh_token` are persisted exclusively
  inside the encrypted `profile.zz` payload. They never touch disk
  in clear and never appear in logs.
- The `Debug` impls on `OneDriveProfile` and `OneDriveAuth` fully
  redact their contents.
- TLS verification is on by default through `ureq`'s defaults; no
  flag bypasses it.
- The Azure AD `client_id` is embedded in the binary. Microsoft's
  guidance for public clients treats it as published metadata; the
  `client_id` only identifies the zz-drop application to Azure and
  does not grant access to user data on its own. There is no
  `client_secret`: public-client device flow does not use one.
- The app is registered as multi-tenant + personal accounts. A user
  signing in delegates only the three scopes listed above; nothing
  else.

## Backward compatibility

The `ProviderProfile` enum gains a `OneDrive` variant alongside
`Nextcloud` and `GoogleDrive`. A profile blob written by a newer
client and opened by an older one fails deserialisation with a
clear error — by design, since the older client cannot honour the
new provider.
