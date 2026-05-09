# Dropbox

zz-drop talks to Dropbox over the Dropbox API v2 and authenticates
with OAuth 2.0 Authorization Code + PKCE in **paste-code mode**.
There is no embedded browser, no local web server, and no
auto-poll: the TUI builds an authorization URL, you open it on any
device with a browser, approve, and Dropbox shows you a short
authorization code on its consent page. You paste that code back
into the terminal and the rest is automatic.

This flow differs from the OneDrive / Google Drive setup in zz-drop
because Dropbox does not implement RFC 8628 device authorization.
The trade-off is one extra step (paste the code) in exchange for
keeping the headless / SSH-friendly behaviour: zz-drop never opens
a local port and never waits for a browser callback.

## Setup

```
zz c
```

In the TUI, choose `Dropbox` from the provider list. zz-drop:

1. Generates a fresh PKCE secret locally (no network round-trip
   yet) and builds an authorize URL.
2. Displays the URL plus a QR encoding it.

Open the URL on a phone or laptop browser (you can scan the QR
with the camera app, or copy/open the URL directly). Dropbox will
ask you to confirm four permissions:

- write to the app's folder (`files.content.write`),
- read from the app's folder (`files.content.read`),
- read metadata for items in the app's folder
  (`files.metadata.read`),
- read your account email (`account_info.read`) — used only to
  display "you upload as alice@example.com" in the zz-drop
  summary.

Once you approve, Dropbox shows you a short authorization code on
the consent page. Copy it, switch back to the TUI, and either
type it in or press **Ctrl-V** to paste from the system clipboard.
Press **Enter** to exchange the code for tokens.

zz-drop then fetches your account email and asks for the local
profile passphrase.

The destination folder for new profiles is the app's sandbox
itself — `Apps/zz-drop/`. zz-drop does not create an extra
nested subfolder underneath, because the App-folder access type
already gives the app a dedicated directory: stacking another
`zz-drop/` inside would surface as `Apps/zz-drop/zz-drop/...`,
which reads like a typo. Profiles created before this default
flipped keep their literal `zz-drop/` subfolder for backward
compatibility.

## Daily use

```
zz file.md          # upload to Apps/zz-drop/file.md
zz d file.md        # download Apps/zz-drop/file.md to ./
```

The output line shows the alias and a compact target string:

```
file.md uploaded · personal · dropbox
```

`dropbox` means the file went to the app's Dropbox sandbox root.
Your Dropbox UI shows the same file under
`Apps/zz-drop/file.md`.

## What zz-drop can and cannot see

zz-drop is registered as an **App folder** app on Dropbox. This is
a structural sandbox — not a permission you can revoke
independently — that limits the binary to a single folder named
after the app.

The app can:

- create, read, update, and delete files and folders inside
  `Apps/zz-drop/`;
- read its own folder's metadata (size, modification time, paths);
- read your account email and basic profile;
- refresh its access while you're offline (the
  `token_access_type=offline` parameter on the authorize URL is
  what makes refresh tokens available).

The app **cannot**:

- list, read, or modify any file outside `Apps/zz-drop/`;
- see your shared links, shared folders, Paper documents, Replay
  projects, or any team / business resources;
- upgrade to a larger scope without you re-consenting at a fresh
  authorize URL.

The OAuth tokens are stored exclusively inside your encrypted
`profile.zz`; they are never written to disk in clear, never
logged, and never sent anywhere except to Dropbox itself.

## Limits

- Single-shot upload caps at **150 MiB** (the Dropbox API limit on
  `/files/upload`). Files larger than 150 MiB are not supported in
  v1; zz-drop returns a clear error rather than starting an upload
  that the API would reject. Resumable upload sessions for larger
  files are tracked as a future enhancement.
- Delete moves files to your Dropbox **trash**, where Dropbox keeps
  them for ~30 days before automatic purging. zz-drop does not
  surface a permanent-delete option in v1; if you need to wipe a
  trashed item earlier, do it from the Dropbox web UI.

## Revocation

To revoke zz-drop's access to your Dropbox account at any time,
visit https://www.dropbox.com/account/connected_apps in your
browser and remove the `zz-drop` entry. Subsequent uploads from
zz-drop will fail with `auth failed` on stderr until you re-run
`zz c` and complete a fresh setup.

## Troubleshooting

- **"oauth flow error: invalid_grant" right after pasting the
  code.** The authorization code expired (Dropbox codes have a
  short lifetime) or was already used. Press `r` to retry the
  setup; zz-drop will generate a fresh URL and PKCE secret.
- **"auth failed" on every upload after a week of working fine.**
  You probably revoked zz-drop's access from the Dropbox web UI,
  or Dropbox invalidated the refresh token for a security
  reason. Re-run `zz c` and pick Dropbox again.
- **"server error: 413" on a large upload.** The file is over
  150 MiB. Split it before uploading, or use a different provider
  in v1.
- **"server error: 429" on rapid uploads.** Dropbox is
  rate-limiting the app. Wait a moment and retry; the limit is
  per-app, not per-user, and zz-drop's behaviour respects it.
