# Nextcloud Login Flow (TUI)

The TUI's auth screen lets the user pick `app password` or `login flow`.
This document describes the **login flow** path — the
[Nextcloud Login Flow v2](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/LoginFlow/index.html)
protocol implemented headless-friendly inside the wizard.

## Why Login Flow

App passwords work but require the user to leave the TUI, sign in via
a browser, generate a token, and paste it back. Login Flow inverts the
problem: the wizard shows a URL (or a QR), the user opens it on any
device and authorizes; meanwhile the TUI polls and grabs the issued
app password automatically. The same `appPassword` ends up in
`profile.zz` either way — only the typing-in step is replaced by a
poll.

## Headless friendliness

zz-drop never tries to open a browser by default. The wizard exposes:

| Key | Action |
|---|---|
| `c` | copy the URL to the clipboard (best-effort) |
| `q` | toggle a QR rendering of the URL |
| `o` | open the URL in a browser **if and only if** the user asks |
| `u` | full-URL detail modal (URL is otherwise truncated to fit) |
| `esc` | cancel the flow and go back to the auth screen |

Wording: there is no default "browser opened" message — the user is
told to "open this link on any device". On a remote SSH session
without a graphical environment, this is exactly the right framing.

## Protocol

Two HTTP calls are made by `zz_drop_core::providers::nextcloud::login_flow`:

1. **Initiate** — `POST {server}/index.php/login/v2`
   ```json
   {
     "poll":  { "token": "...", "endpoint": "..." },
     "login": "https://nc.example.org/index.php/login/v2/flow/<token>"
   }
   ```
2. **Poll** — `POST {endpoint}` with form body `token=<token>`
   - `200` + `{ server, loginName, appPassword }` → success
   - `404` + empty body → not yet, keep polling
   - anything else → server / network error

The TUI polls every 2 seconds while the screen is active. `Esc`
cancels the flow without closing the TUI.

## Inline image vs ASCII fallback

When you press `q` the wizard tries to show the URL as a real
**inline image** in the terminal:

- iTerm2, Kitty, WezTerm, Alacritty + Sixel, etc. → the QR is drawn as
  a crisp PNG-like image scanned cleanly by any phone camera.
- xterm, GNU/Linux console, plain SSH → the QR falls back to a
  half-block ASCII rendering (`▀ ▄ █ ` characters). Still scannable
  by most phones, just chunkier.

Detection happens **once at startup** by querying the terminal via
stdio (before entering the alternate screen). Result is held in
`App::graphics: Option<GraphicsCtx>`. If detection times out or fails,
the wizard transparently uses the ASCII renderer — no error is shown.

Implementation entry points:

- `zz_drop_tui::qr::GraphicsCtx::detect()` — terminal probe
- `zz_drop_tui::qr::render_qr_image(...)` — inline image path
- `zz_drop_tui::qr::render_qr(...)` — half-block ASCII fallback

## Clipboard fallback

`c` calls `arboard::Clipboard::new`. On a headless server with no X
server / Wayland the call fails — the wizard catches that and shows
"clipboard not available" inline, **without** panicking. Manual copy
via `u` (URL detail modal) remains available.

## Browser open fallback

`o` calls `open::that_detached`. On a headless box this typically
fails too; the wizard surfaces "could not open browser" inline. The
failure is non-fatal; the user can still copy or scan.

## Security

- `LoginFlowResult` (the JSON returned at success) holds the
  `appPassword`. Its `Debug` impl is hand-rolled and prints
  `LoginFlowResult { <redacted> }` so the value cannot leak into
  panics, logs or terminal echo via `{:?}`.
- The token in `LoginFlowState::poll_token` is similarly redacted by
  `LoginFlowState`'s custom `Debug`.
- TLS verification on the Nextcloud HTTPS request is on by default
  (rustls + webpki-roots).
- Once the flow succeeds, the `appPassword` is written into
  `WizardState::auth_secret`. From that point onward the wizard
  treats the credential exactly as if the user had pasted an app
  password manually — same masked render in the auth screen, same
  encryption path through `profile.zz` in TASK 14.

## Cancellation and re-entry

Press `esc` at any time during the flow to cancel. The poll loop
stops and the wizard returns to the auth screen with credentials
cleared. Press `enter` from the auth screen again to start over —
fresh token, fresh URL, fresh poll loop.

## Test coverage

`zz-drop-tui/tests/login_flow_ui.rs` exercises the state machine
without touching the network: navigation in/out of the screen,
`q`/`u`/`o`/`c` key dispatch, modal handling, redaction of the
`apply_login_flow_done` credentials. The actual HTTP round-trip is
unit-tested in `zz-drop-core/src/providers/nextcloud/login_flow.rs`
on representative server payloads.
