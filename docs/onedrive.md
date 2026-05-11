# OneDrive

zz-drop talks to OneDrive over Microsoft Graph and authenticates
with OAuth 2.0 Device Authorization Grant — the same flow TVs and
game consoles use. There is no embedded browser, no local web
server, and no token paste-back: the TUI shows a short code, you
open the URL on any device with a browser, approve, and the
terminal continues automatically.

The same setup works for both **personal Microsoft accounts**
(`@outlook.com`, `@hotmail.com`, `@live.com`, …) and **work or
school accounts** managed by an organisation tenant.

## Setup

```
zz c
```

In the TUI, choose `OneDrive` from the provider list. zz-drop:

1. Contacts Microsoft and obtains a one-time `user_code` and a
   verification URL (`https://login.microsoft.com/device`).
2. Displays both, plus a QR code that encodes the URL with the
   code already filled in.
3. Polls Microsoft in the background.

Approve the consent on a phone or laptop browser (you can scan
the QR with the camera app, or open the URL and type the code
manually). Microsoft will ask you to confirm three permissions:

- read and write your OneDrive files,
- see your basic profile (used to display your email in the
  zz-drop summary),
- maintain access while you're not actively signed in (this is
  what enables the offline refresh token).

Once approved, zz-drop fetches your account email so it can show
`you upload as alice@example.org` in the profile summary, then
asks for the local profile passphrase. (`example.org` is just a
placeholder; the real label uses your actual Microsoft account
address.)

The folder name defaults to `zz-drop`. Change it before saving the
profile if you want to use a different one — once the profile is
saved, the folder is created on your OneDrive on the first upload.

## Daily use

```
zz file.md          # upload to <root>/file.md
zz d file.md        # download <root>/file.md to ./
```

The output line shows the alias and a compact target string:

```
file.md uploaded · personal · onedrive/zz-drop
```

`onedrive/zz-drop` means the file went to the `zz-drop` folder on
your OneDrive. Replace the path with whatever you set during
setup.

## What zz-drop can and cannot see

zz-drop requests three delegated scopes:
`Files.ReadWrite`, `User.Read`, `offline_access`.

The app can:

- create, read, update, and delete files and folders in your
  OneDrive;
- read your account's `mail` / `userPrincipalName` for the display
  label;
- refresh its access while you're offline (the `offline_access`
  scope).

The app **cannot**:

- read your other Microsoft 365 services (Outlook mail, Teams
  messages, Calendar, …): only OneDrive content is in scope;
- act on behalf of other users in your tenant;
- modify any directory / tenant settings.

`Files.ReadWrite` is broader than Google Drive's equivalent
`drive.file` — Microsoft Graph does not offer a "files created
by this app only" scope at the same granularity. If you want to
isolate zz-drop's footprint, create the `zz-drop/` folder
manually first and only put files inside that folder; everything
zz-drop touches will live under it because that is the
configured root.

## Where the tokens live

Both the access and refresh tokens are stored inside the encrypted
`profile.zz` payload, never on disk in clear. The profile is
decrypted only into the local zz-drop agent's RAM; locking the
profile (`zz q`) wipes them from memory.

zz-drop refreshes the access token automatically when it has 60 s
or less left. If the refresh fails — typically because you revoked
zz-drop's access from your Microsoft account, or because your
organisation's conditional-access policy now blocks it — `zz`
exits with `token expired` on stderr, and you can re-run setup to
issue fresh tokens.

## Limits in v1

- Uploads ≤ 4 MiB use a single PUT; larger files use a chunked
  upload session. Both are robust to the typical zz-drop file
  sizes (notes, screenshots, configs, modest archives).
- Concurrent uploads under the same alias are not supported; run
  `zz` calls sequentially.
- Move and rename are not exposed via the CLI in v1; uploads
  create fresh entries, downloads pull by name.
- `Files.ReadWrite` (rather than the more restricted AppFolder
  scope) is used because Graph's app-folder permission model is
  significantly less ergonomic than Drive's `drive.file`. We may
  re-evaluate in a future release.

## Business tenants and Microsoft "verified publisher"

If you sign in with a **work or school account** and your
organisation enforces the [risk-based step-up consent
policy](https://learn.microsoft.com/entra/identity/enterprise-apps/configure-risk-based-step-up-consent),
the Microsoft consent screen may refuse to let the OneDrive setup
proceed with a message like *"end users can't grant consent to
this app because the publisher isn't verified"*. This is a
Microsoft Entra policy that blocks any multitenant app registered
after November 8, 2020 from being consented to by users outside
its home tenant unless the publisher is verified through
Microsoft's [Cloud Partner Program](https://learn.microsoft.com/entra/identity-platform/publisher-verification-overview).

zz-drop's default OneDrive `client_id` is **not** publisher-verified
and will not be — Microsoft's CPP enrolment is built around
"legitimate business entities" with formal registration and
domain-bound email, which is a poor fit for a single-maintainer
open-source CLI. The same posture applies to
[rclone](https://rclone.org/onedrive/), whose docs describe the
identical workaround.

**The workaround is to register your own Microsoft client_id.**
Five minutes in the Azure portal: you create a one-off
multitenant app on your own account (or your organisation's
tenant), enable public-client flows, and rebuild zz-drop with
the resulting `client_id` baked in via the build-time override
`ZZ_DROP_ONEDRIVE_CLIENT_ID`. Step-by-step in
[`docs/build.md`](build.md#building-with-your-own-oauth-client-ids).

The default `client_id` works fine for personal Microsoft
accounts (`@outlook.com`, `@hotmail.com`, `@live.com`) and for
business tenants that haven't enabled the step-up consent policy
— this section only matters when the consent screen actively
blocks you.

## Revoking access

To withdraw zz-drop's access at any time:

- **Personal account**: visit
  `https://account.live.com/consent/Manage` and remove zz-drop.
- **Work / school account**: visit
  `https://myapps.microsoft.com` and remove zz-drop, or ask your
  tenant admin to revoke consent.

After revocation, the existing tokens stop working on the next
refresh and the CLI surfaces `token expired`. Run `zz w` if you
also want to wipe the local profile state.
