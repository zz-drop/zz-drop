// Standalone QR-generation smoke test. No ratatui, no terminal alt-screen,
// no inline-image protocol — just prints the half-block QR for a typical
// Nextcloud Login Flow URL straight to stdout.
//
// If you don't see a QR after running this, the qrcode crate or the
// rendering code is broken. If you DO see a QR here but not in `zz-tui`,
// the problem is with how ratatui composes the buffer.

fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://cloud.example.org/index.php/login/v2/flow/abcdef0123456789".into());

    println!("URL ({}  chars): {}", url.chars().count(), url);
    println!();

    let code = match qrcode::QrCode::new(url.as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("qrcode::QrCode::new failed: {e:?}");
            std::process::exit(1);
        }
    };

    let width = code.width();
    let modules: Vec<bool> = code
        .to_colors()
        .iter()
        .map(|c| *c == qrcode::Color::Dark)
        .collect();

    let half_h = width.div_ceil(2);
    println!("modules: {width} x {width}   ascii: {} x {}", width + 2, half_h);
    println!();

    // Top quiet zone
    println!("{}", " ".repeat(width + 2));

    for row in 0..half_h {
        let y_top = row * 2;
        let y_bot = y_top + 1;
        let mut s = String::with_capacity(width + 2);
        s.push(' ');
        for x in 0..width {
            let top = modules[y_top * width + x];
            let bot = if y_bot < width {
                modules[y_bot * width + x]
            } else {
                false
            };
            s.push(match (top, bot) {
                (true, true) => '\u{2588}',  // █
                (true, false) => '\u{2580}', // ▀
                (false, true) => '\u{2584}', // ▄
                (false, false) => ' ',
            });
        }
        s.push(' ');
        println!("{s}");
    }
    println!("{}", " ".repeat(width + 2));
    println!();
    println!("if you can scan this with your phone, qrcode + half-block render are fine.");
}
