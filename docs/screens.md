# `zz-tui` screen catalogue

This is the per-screen reference. For the **flow** narrative (which
screen leads to which), see [`setup.md`](setup.md). For the
**Login Flow** specifics, see [`login-flow.md`](login-flow.md). For
the **profile passphrase** specifics, see
[`profile-passphrase.md`](profile-passphrase.md).

Every screen below is rendered with the same chrome:

- a **title bar** showing `▍ zz-tui  ›  <breadcrumb>` on the left
  and the agent-status pill on the right
- a **stepper band** (`welcome / provider / server / auth /
  folder / encrypt / done`), shown on every screen except Welcome
  and Done
- a **body** drawn by the screen
- a **keybar** along the bottom with chip-style key + label pairs

The 8-step indices are mapped from `Screen::stepper_index()` in
`src/screens/mod.rs`.

## Setup wizard

| Screen | Breadcrumb | Step | Body |
|---|---|---|---|
| `Welcome` | `welcome` | – | Tagline + 3-row menu (Configure / Recover / Quit). Recover is a stub that points users at the CLI command `zz z <alias>`. |
| `Provider` | `setup › provider` | 1 | Single Mint panel with five radio rows (Nextcloud · Google Drive · OneDrive · Dropbox active; Proton disabled with a `(planned)` hint). |
| `NextcloudServer` | `setup › server` | 2 | URL form field, validation tag (`[v URL parses]` / `[x not a URL]`). Pre-filled with `https://`. |
| `NextcloudAuth` | `setup › auth` | 3 | Two-column: left Mint panel with method radios + (when App Password) username/secret form fields. Right Cyan panel with method-specific copy. |
| `NextcloudLoginFlow` | `setup › auth › login flow` | 3 | See [`login-flow.md`](login-flow.md). Two-col: QR panel left, link/status panel right. Single-col fallback when terminal is narrow. |
| `RemoteFolder` | `setup › folder` | 4 | Path form field with core-validated path tag. |
| `Collision` | `setup › collision` | 4 | Three radio rows (Rename / Overwrite / Fail) + a Dim "preview" panel that adapts to the focused choice. |
| `TestUpload` | `setup › probe` | 4 | Three Check rows (`ensure folder` / `upload tiny file` / `cleanup`). Two-stage probe with live progress; cleanup currently shows `Skip` until follow-up work lands. |
| `InnerAlias` | `setup › alias` | – | Single bordered input pre-filled with a `<provider>-<adj>-<noun>-NN` mnemonic suggestion. Reached after `TestUpload` in every wizard mode (first-profile + add-inner) so the operator always picks a deliberate alias instead of inheriting a placeholder. `Tab` regenerates the suggestion; `Enter` confirms. From here the first-profile flow advances to `ProfilePassphrase`; the add-inner flow appends to the unlocked container directly. |
| `ProfilePassphrase` | `setup › encrypt` | 5 | See [`profile-passphrase.md`](profile-passphrase.md). Two masked form fields, zxcvbn strength bar, weak-passphrase warning sub-state. |
| `Done` | `done` | 7 | CLI cheat sheet. |

## Common keybindings

| Key | Action |
|---|---|
| `Enter` | confirm / advance |
| `Esc` | back, or quit on Welcome |
| `↑` `↓` | move between options (radios, menu items, alias picker) |
| `Tab` / `Shift+Tab` | cycle between fields (auth, passphrase, account) |
| `Ctrl+C` | quit anywhere |

Per-screen extras live in the keybar at the bottom of the screen
itself. Notable screen-specific bindings:

- **Login Flow** — `c` copy URL · `o` open in browser · `u` show
  full URL modal · `q` toggle QR · `i` toggle inline/ASCII QR ·
  `r` retry on failure
- **Test upload** — `↵` runs the probe when idle, advances on
  success, retries on failure
- **Profile passphrase (weak warning)** — `y` continue anyway,
  `n` go back to editing
- **Done** — `q` exit

## Layout primitives

The screens use a small set of widget primitives in
`src/tui_widgets/`:

| Primitive | Used for |
|---|---|
| `panel::open` | every framed body region (returns inner Rect) |
| `form_field::render` | bordered text input with a 14-cell label |
| `radio::render_row` | provider / collision / auth-method choices |
| `check::render_row` | TestUpload steps and Login Flow status |
| `tag::render` | small `[v ok]` / `[x bad]` validation badges |
| `keybar::render` | bottom chip-style key/label row |
| `steps::render` | top stepper band with dot glyphs |
| `title_bar::render` | wordmark + breadcrumb + agent pill |

Each primitive lives in its own file and ships with at least one
unit test that asserts a known glyph or a buffer-contents invariant.

## Render fallbacks

| Condition | What happens |
|---|---|
| Terminal smaller than `MIN_WIDTH × MIN_HEIGHT` | centered `"terminal too small (WxH), need 80x24"` message; no panic |
| `NO_COLOR=1` | `Color::Rgb` accessors degrade to `BOLD`/`DIM`/`UNDERLINED`/`REVERSED` modifiers; all status remains readable |
| Long URL on Login Flow | middle-ellipsis truncation in the panel; full URL available in the `u` modal |
| Inline-image QR not reliable on the host terminal | half-block ASCII fallback; pane width auto-sized so the QR keeps a 1:1 aspect ratio |
