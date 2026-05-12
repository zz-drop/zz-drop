use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{CheckStatus, KeyHint, check, panel};
use crate::wizard::{ProbeProgress, ProbeStepStatus, TestOutcome};

pub struct TestUploadScreen;

impl TestUploadScreen {
    pub fn title() -> &'static str {
        "test upload"
    }

    pub fn keybar_hint(running: bool, outcome: Option<&TestOutcome>) -> Vec<KeyHint> {
        if running {
            return vec![KeyHint::new("…", "probing")];
        }
        match outcome {
            Some(TestOutcome::Ok) => vec![
                KeyHint::new("↵", "continue"),
                KeyHint::new("esc", "back"),
            ],
            Some(TestOutcome::Failed(_)) => vec![
                KeyHint::new("↵", "retry"),
                KeyHint::new("esc", "back"),
            ],
            None => vec![
                KeyHint::new("↵", "run probe"),
                KeyHint::new("esc", "back"),
            ],
        }
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        progress: &ProbeProgress,
        outcome: Option<&TestOutcome>,
        running: bool,
    ) {
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, " probe upload ");
        if inner.height < 6 {
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        let intro = match (running, outcome) {
            (true, _) => "  probing the remote folder…",
            (false, Some(TestOutcome::Ok)) => "  ready · press enter to continue.",
            (false, Some(TestOutcome::Failed(_))) => "  press enter to retry.",
            (false, None) => "  press enter to probe the remote folder.",
        };
        let header = Paragraph::new(vec![
            Line::from(Span::styled(intro, theme.dim())),
            Line::from(Span::styled(
                "  zz-drop checks the folder, leaves a marker, then writes & deletes a test file.",
                theme.dim(),
            )),
        ]);
        ratatui::widgets::Widget::render(header, rows[0], buf);

        check::render_row(
            rows[1],
            buf,
            theme,
            map_status(progress.ensure),
            "ensure folder",
            Some("PROPFIND + MKCOL"),
        );
        check::render_row(
            rows[2],
            buf,
            theme,
            map_status(progress.marker),
            "leave marker",
            Some("Halvdan_was_here · delete it when you no longer need it"),
        );
        check::render_row(
            rows[3],
            buf,
            theme,
            map_status(progress.upload),
            "upload test file",
            Some("PUT"),
        );
        check::render_row(
            rows[4],
            buf,
            theme,
            map_status(progress.cleanup),
            "cleanup",
            Some("DELETE"),
        );

        // Footer: "ok" in green or the failure reason in red. While the
        // probe is running we leave it blank so the eye tracks the moving
        // ◌ glyph instead.
        if !running {
            if let Some((msg, style)) = match outcome {
                Some(TestOutcome::Ok) => Some(("ok", theme.ok())),
                Some(TestOutcome::Failed(reason)) => Some((reason.as_str(), theme.danger())),
                None => None,
            } {
                let footer =
                    Paragraph::new(Line::from(Span::styled(format!("    {msg}"), style)));
                ratatui::widgets::Widget::render(footer, rows[6], buf);
            }
        }
    }
}

fn map_status(s: ProbeStepStatus) -> CheckStatus {
    match s {
        ProbeStepStatus::Pending => CheckStatus::Skip,
        ProbeStepStatus::Busy => CheckStatus::Busy,
        ProbeStepStatus::Ok => CheckStatus::Ok,
        ProbeStepStatus::Err => CheckStatus::Err,
        ProbeStepStatus::Skip => CheckStatus::Skip,
    }
}
