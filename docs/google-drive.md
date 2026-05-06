# Google Drive

zz-drop talks to Google Drive over the official Drive v3 API and
authenticates with OAuth 2.0 Device Authorization Grant — the same
flow TVs and game consoles use. There is no embedded browser, no
local web server, and no token paste-back: the TUI shows a short
code, you open the URL on any device with a browser, approve, and
the terminal continues automatically.

## Setup

```
zz c
```

In the TUI, choose `Google Drive` from the provider list. zz-drop:

1. Contacts Google and obtains a one-time `user_code` and a
   `verification_url`.
2. Displays both, plus a QR code that encodes the URL with the code
   already filled in.
3. Polls Google in the background.

Approve the consent on a phone or laptop browser (you can scan the
QR with the camera app). Once approved, zz-drop fetches your Google
account email so it can show `you upload as alice@example.com` in
the profile summary, then asks for the local profile passphrase.

The folder name defaults to `zz-drop`. Change it before saving the
profile if you want to use a different one — once the profile is
saved, the folder is created on your Drive on the first upload.

## Daily use

```
zz file.md          # upload to <root>/file.md
zz d file.md        # download <root>/file.md to ./
```

The output line shows the alias and a compact target string:

```
file.md uploaded · personal · gdrive/zz-drop
```

`gdrive/zz-drop` means the file went to the `zz-drop` folder on
Google Drive. Replace the path with whatever you set during setup.

## What zz-drop can and cannot see

zz-drop requests only the `drive.file` OAuth scope. The app can:

- create, read, update, and delete files it created;
- create, read, update, and delete folders it created;
- read files you explicitly open with the app (not used in v1).

The app **cannot**:

- list or read any other Drive file you have;
- list or read any folder it didn't create itself;
- read your account profile beyond the email address used for the
  display label.

If you happen to have an existing folder named `zz-drop` on your
Drive, zz-drop won't see it — it will create its own folder under
the same name. The two coexist.

## Where the tokens live

Both the access and refresh tokens are stored inside the encrypted
`profile.zz` payload, never on disk in clear. The profile is
decrypted only into the local zz-drop agent's RAM; locking the
profile (`zz q`) wipes them from memory.

zz-drop refreshes the access token automatically when it has 60 s
or less left. If the refresh fails — typically because you revoked
zz-drop's access from your Google account — `zz` exits with
`token expired` on stderr, and you can re-run setup to issue fresh
tokens.

## Limits in v1

- Uploads use the simple multipart format. For typical zz-drop
  files (notes, screenshots, configs, modest archives) this is
  fine. Very large files (multi-hundred MB and up) can fail or be
  slow until resumable upload lands in a follow-up release.
- Concurrent uploads under the same alias are not supported; run
  `zz` calls sequentially.
- Move and rename are not exposed via the CLI in v1; uploads create
  fresh entries, downloads pull by name.

## Revoking access

To withdraw zz-drop's access at any time, visit Google's account
permissions page (`https://myaccount.google.com/permissions`) and
remove zz-drop. After that, the existing tokens stop working on the
next refresh and the CLI surfaces `token expired`. Run `zz w` if
you also want to wipe the local profile state.
