//! Ratatui-flavored design primitives mirroring `DESIGN/tui.jsx`.
//! Each widget is a free function; primitives don't own state and are
//! cheap to compose into a screen body.

pub mod bar;
pub mod check;
pub mod form_field;
pub mod glyphs;
pub mod keybar;
pub mod nav_item;
pub mod panel;
pub mod pill;
pub mod radio;
pub mod steps;
pub mod tag;
pub mod title_bar;
pub mod tui_btn;
pub mod two_col;

pub use check::CheckStatus;
pub use keybar::KeyHint;
pub use pill::AgentPill;
pub use steps::StepState;
pub use tag::TagKind;
