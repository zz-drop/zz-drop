# Ratatui notes

`zz-tui` runs entirely in the terminal — no GUI, no `xdg-open`, no
embedded browser. Constraints we hold ourselves to:

- Target frame **100 × 30**; minimum **80 × 24**. Smaller terminals
  show a centered "terminal too small" message instead of panicking.
- No floating elements outside panel rectangles. No popovers; modals
  (e.g. the Login Flow URL detail) take over the body region.
- Long URLs and paths are truncated with a middle ellipsis or shown
  in a dedicated modal — never wrapped onto an unpredictable number
  of lines.
- The bottom **keybar is always visible** and reflects the current
  state of the screen.
- **Color is never required for meaning**. With `NO_COLOR=1` the
  TUI degrades to `BOLD` / `DIM` / `UNDERLINED` / `REVERSED` and
  every status remains readable.
- **Login Flow is headless-friendly**: a phone is enough to complete
  authentication; no browser on the box running `zz-tui` is required.
- The QR renderer falls back to half-block ASCII whenever inline
  graphics aren't reliably available (see the QR rendering section
  in [`../README.md`](../README.md)).
