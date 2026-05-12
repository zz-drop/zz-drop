use ratatui::Terminal;
use ratatui::backend::TestBackend;
use zz_drop_tui::app::App;
use zz_drop_tui::screens::Screen;
use zz_drop_tui::theme::{MockEnv, Theme};
use zz_drop_tui::ui;
use zz_drop_tui::wizard::LoginFlowStage;

fn main() {
    let theme = Theme::from_parts(&MockEnv::empty(), false);
    for (w, h) in [(80, 24), (100, 30), (120, 32)] {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.screen = Screen::NextcloudLoginFlow;
        app.login_flow.stage = LoginFlowStage::Polling;
        // Realistic Nextcloud Login Flow URL — 53 QR modules, overflows
        // half-block at the 44-col left pane and forces the quadrant fallback.
        app.login_flow.login_url =
            "https://nextcloud.example.org/index.php/login/v2/flow/aBcD3FgHiJkLmNoPqRsTuVwXyZ0123456789aBcDeFgHiJkLmNoPqRsTu".into();
        app.login_flow.disable_inline_qr = true;
        term.draw(|frame| ui::draw(frame, &mut app, &theme)).unwrap();
        println!("\n──── frame {w}x{h} ────");
        let buf = term.backend().buffer();
        for y in 0..h {
            let mut line = String::with_capacity(w as usize);
            for x in 0..w {
                let s = buf[(x, y)].symbol();
                line.push_str(if s.is_empty() { " " } else { s });
            }
            println!("{}", line.trim_end());
        }
    }
}
