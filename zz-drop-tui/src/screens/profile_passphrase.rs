use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

use crate::input::TextInput;
use crate::strength::evaluate;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{CheckStatus, KeyHint, bar, check, form_field, panel, two_col};
use crate::wizard::{PassphraseFocus, PassphraseStage};

pub struct ProfilePassphraseScreen;

impl ProfilePassphraseScreen {
    pub fn title() -> &'static str {
        "profile passphrase"
    }

    pub fn keybar_hint(stage: &PassphraseStage) -> Vec<KeyHint> {
        match stage {
            PassphraseStage::Editing => vec![
                KeyHint::new("tab", "field"),
                KeyHint::new("↵", "save"),
                KeyHint::new("esc", "back"),
            ],
            PassphraseStage::WeakWarning => vec![
                KeyHint::new("y", "continue anyway"),
                KeyHint::new("n", "edit"),
                KeyHint::new("esc", "back"),
            ],
            PassphraseStage::Encrypting => vec![KeyHint::new("…", "encrypting")],
            PassphraseStage::Saved(_) => vec![
                KeyHint::new("p", "push to server"),
                KeyHint::new("↵", "skip · local-only"),
                KeyHint::new("q", "quit"),
            ],
            PassphraseStage::Failed(_) => vec![
                KeyHint::new("↵", "retry"),
                KeyHint::new("esc", "back"),
            ],
        }
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        passphrase: &TextInput,
        confirm: &TextInput,
        focus: PassphraseFocus,
        stage: &PassphraseStage,
        server_label: &str,
    ) {
        match stage {
            PassphraseStage::Encrypting => render_full_panel(
                area,
                buf,
                theme,
                PanelAccent::Mint,
                " profile passphrase ",
                vec![
                    Line::from(""),
                    Line::from(Span::styled("  encrypting profile…", theme.accent_bold())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Argon2id ~500 ms – 1 s — this is intentional.",
                        theme.dim(),
                    )),
                ],
            ),
            PassphraseStage::Saved(path) => render_full_panel(
                area,
                buf,
                theme,
                PanelAccent::Mint,
                " profile saved ",
                vec![
                    Line::from(""),
                    Line::from(Span::styled("  ✓ profile saved", theme.ok())),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  ", theme.dim()),
                        Span::styled(path.clone(), theme.cyan()),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  the encrypted blob is on this machine only.",
                        theme.body(),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  push it to ", theme.body()),
                        Span::styled(server_label.to_string(), theme.cyan()),
                        Span::styled(" so it can be retrieved", theme.body()),
                    ]),
                    Line::from(Span::styled(
                        "  from any shell with `zz z <alias>`?",
                        theme.body(),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  press ", theme.dim()),
                        Span::styled("p", theme.accent_bold()),
                        Span::styled(" to push, ", theme.dim()),
                        Span::styled("↵", theme.accent_bold()),
                        Span::styled(" to skip (local-only).", theme.dim()),
                    ]),
                ],
            ),
            PassphraseStage::Failed(reason) => render_full_panel(
                area,
                buf,
                theme,
                PanelAccent::Red,
                " profile passphrase ",
                vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  ✗ save failed: {reason}"),
                        theme.danger(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  press esc to go back and try again.",
                        theme.dim(),
                    )),
                ],
            ),
            PassphraseStage::Editing => {
                render_editing(area, buf, theme, passphrase, confirm, focus, false, server_label);
            }
            PassphraseStage::WeakWarning => {
                render_editing(area, buf, theme, passphrase, confirm, focus, true, server_label);
            }
        }
    }
}

fn render_full_panel(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    accent: PanelAccent,
    title: &str,
    lines: Vec<Line<'_>>,
) {
    let inner = panel::open(area, buf, theme, accent, title);
    if inner.height < 3 {
        return;
    }
    let p = Paragraph::new(lines);
    ratatui::widgets::Widget::render(p, inner, buf);
}

fn render_editing(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    passphrase: &TextInput,
    confirm: &TextInput,
    focus: PassphraseFocus,
    weak_warning: bool,
    server_label: &str,
) {
    let (left_area, right_area) = two_col::split(area, 56, 2);
    let two_col_active = right_area.width > 0;

    // ─── left panel: passphrase form ─────────────────────────────
    let left_inner =
        panel::open(left_area, buf, theme, PanelAccent::Mint, " profile passphrase ");
    if left_inner.height >= 6 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(left_inner);

        form_field::render(
            rows[0],
            buf,
            theme,
            "passphrase",
            passphrase,
            focus == PassphraseFocus::Passphrase,
        );
        form_field::render(
            rows[1],
            buf,
            theme,
            "confirm",
            confirm,
            focus == PassphraseFocus::Confirm,
        );

        // strength bar
        let strength = evaluate(passphrase.value());
        let bar_area = Rect::new(rows[2].x + 2, rows[2].y, rows[2].width.saturating_sub(2), 1);
        bar::render(bar_area, buf, theme, strength.score, 4, 16, strength.label);

        // match check
        let pass_eq = passphrase.value() == confirm.value();
        let check_area = Rect::new(
            rows[3].x + 2,
            rows[3].y,
            rows[3].width.saturating_sub(2),
            1,
        );
        if !confirm.value().is_empty() && !pass_eq {
            check::render_row(check_area, buf, theme, CheckStatus::Err, "passphrases do not match", None);
        } else if !passphrase.value().is_empty() && pass_eq {
            check::render_row(check_area, buf, theme, CheckStatus::Ok, "passphrases match", None);
        } else {
            check::render_row(check_area, buf, theme, CheckStatus::Skip, "type a passphrase", None);
        }
    }

    // ─── right panel: warning OR recommendations ────────────────
    if !two_col_active {
        return;
    }
    if weak_warning {
        let inner = panel::open(
            right_area,
            buf,
            theme,
            PanelAccent::Yellow,
            " weak passphrase ",
        );
        if inner.height < 4 {
            return;
        }
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "if someone gets profiles-local.zz, they can try an offline attack.",
                theme.body(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "recommended: 12+ characters.",
                theme.dim(),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("press ", theme.dim()),
                Span::styled("y", theme.warn()),
                Span::styled(" to continue, ", theme.dim()),
                Span::styled("n", theme.dim()),
                Span::styled(" to edit.", theme.dim()),
            ]),
        ];
        let p = Paragraph::new(lines)
            .block(Block::new().padding(Padding::horizontal(2)))
            .wrap(Wrap { trim: true });
        ratatui::widgets::Widget::render(p, inner, buf);
    } else {
        // Strong "no recovery" notice. The wizard's output is a
        // local-only blob (`profile-local.zz`); unless the operator
        // later signs in to a server and pushes it, losing either
        // the file or the passphrase means losing the contents.
        let inner = panel::open(
            right_area,
            buf,
            theme,
            PanelAccent::Yellow,
            " before you continue ",
        );
        if inner.height < 4 {
            return;
        }
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "⚠  this is a local-only profile.",
                theme.warn(),
            )),
            Line::from(""),
            Line::from(Span::styled("if you lose either:", theme.body())),
            Line::from(vec![
                Span::styled("  • the file ", theme.body()),
                Span::styled("profiles-local.zz", theme.cyan()),
            ]),
            Line::from(Span::styled(
                "  • the passphrase you pick now",
                theme.body(),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("the contents are gone — ", theme.body()),
                Span::styled("there is no recovery.", theme.warn()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("to make it recoverable, sign in to ", theme.dim()),
                Span::styled(server_label.to_string(), theme.cyan()),
                Span::styled(
                    " afterwards and push it under an alias.",
                    theme.dim(),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "12+ characters recommended.",
                theme.dim(),
            )),
        ];
        let p = Paragraph::new(lines)
            .block(Block::new().padding(Padding::horizontal(2)))
            .wrap(Wrap { trim: true });
        ratatui::widgets::Widget::render(p, inner, buf);
    }
}
