use zz_drop::color::{ColorPolicy, MockEnv};
use zz_drop::output::{
    TargetLabel, human_size, render_failed, render_hint, render_list_entry, render_uploaded,
};

fn off() -> ColorPolicy {
    ColorPolicy::from_parts(&MockEnv::empty(), false)
}

fn on_via_force() -> ColorPolicy {
    ColorPolicy::from_parts(&MockEnv::empty().with("FORCE_COLOR", "1"), false)
}

fn scope() -> TargetLabel<'static> {
    TargetLabel {
        alias: "casa-nc",
        target: "cloud.example.org/zz-drop",
    }
}

#[test]
fn uploaded_plain_format_no_ansi_when_off() {
    let c = off();
    let out = render_uploaded("readme.md", "12 KiB", None, scope(), &c);
    assert_eq!(out, "uploaded readme.md 12 KiB → casa-nc · cloud.example.org/zz-drop");
    assert!(!out.contains('\x1b'));
}

#[test]
fn uploaded_has_ansi_when_on() {
    let c = on_via_force();
    let out = render_uploaded("readme.md", "12 KiB", None, scope(), &c);
    assert!(out.contains('\x1b'), "expected ANSI escape: `{out}`");
    assert!(out.contains("uploaded"));
    assert!(out.contains("readme.md"));
    assert!(out.contains("12 KiB"));
    assert!(out.contains("casa-nc"));
    assert!(out.contains("cloud.example.org/zz-drop"));
}

#[test]
fn uploaded_with_compression_appends_ratio_after_size() {
    let c = off();
    let out = render_uploaded("photo.png.zst", "1.4 MiB", Some(10), scope(), &c);
    assert_eq!(
        out,
        "uploaded photo.png.zst 1.4 MiB (10% compressed) → casa-nc · cloud.example.org/zz-drop"
    );
}

#[test]
fn uploaded_without_compression_omits_ratio_suffix() {
    let c = off();
    let out = render_uploaded("readme.md", "12 KiB", None, scope(), &c);
    assert!(!out.contains("compressed"));
}

#[test]
fn failed_plain_format_no_ansi_when_off() {
    let c = off();
    let out = render_failed("readme.md", "locked", None, &c);
    assert_eq!(out, "failed readme.md locked");
    assert!(!out.contains('\x1b'));
}

#[test]
fn failed_with_scope_includes_alias_and_target() {
    let c = off();
    let out = render_failed("readme.md", "too big", Some(scope()), &c);
    assert_eq!(
        out,
        "failed readme.md too big (casa-nc · cloud.example.org/zz-drop)"
    );
}

#[test]
fn failed_has_ansi_when_on() {
    let c = on_via_force();
    let out = render_failed("readme.md", "locked", None, &c);
    assert!(out.contains('\x1b'));
    assert!(out.contains("failed"));
    assert!(out.contains("readme.md"));
    assert!(out.contains("locked"));
}

#[test]
fn hint_format() {
    assert_eq!(render_hint("zz x"), "run: zz x");
}

#[test]
fn list_entry_uses_single_quotes_for_path() {
    let row = render_list_entry("@casa-nc", "2 KiB", "docs/readme.md");
    assert!(row.contains("'docs/readme.md'"));
    assert!(row.starts_with("@casa-nc"));
    assert!(!row.contains('\x1b'));
}

#[test]
fn list_entry_handles_dash_size_for_directories() {
    let row = render_list_entry("@casa-nc", "-", "docs/");
    assert!(row.contains("'docs/'"));
}

#[test]
fn list_entry_no_ansi() {
    let row = render_list_entry("@casa-nc", "12 KiB", "leggimi.txt");
    assert!(!row.contains('\x1b'));
}

#[test]
fn human_size_below_kib_is_bytes() {
    assert_eq!(human_size(0), "0 B");
    assert_eq!(human_size(1), "1 B");
    assert_eq!(human_size(1023), "1023 B");
}

#[test]
fn human_size_kib_with_decimal_when_small() {
    assert_eq!(human_size(1024), "1.0 KiB");
    assert_eq!(human_size(1536), "1.5 KiB");
}

#[test]
fn human_size_kib_integer_when_large() {
    assert_eq!(human_size(12_345), "12 KiB");
}

#[test]
fn human_size_mib() {
    assert_eq!(human_size(1_500_000), "1.4 MiB");
    assert_eq!(human_size(50_000_000), "47 MiB");
}

#[test]
fn human_size_gib() {
    assert_eq!(human_size(2_500_000_000), "2.3 GiB");
}

#[test]
fn no_color_env_strips_ansi_even_with_tty() {
    let p = ColorPolicy::from_parts(&MockEnv::empty().with("NO_COLOR", "1"), true);
    let out = render_uploaded("a", "1 B", None, scope(), &p);
    assert!(!out.contains('\x1b'));
}

#[test]
fn clicolor_zero_strips_ansi_even_with_tty() {
    let p = ColorPolicy::from_parts(&MockEnv::empty().with("CLICOLOR", "0"), true);
    let out = render_failed("a", "x", None, &p);
    assert!(!out.contains('\x1b'));
}

#[test]
fn force_color_emits_ansi_without_tty() {
    let p = ColorPolicy::from_parts(&MockEnv::empty().with("FORCE_COLOR", "1"), false);
    let out = render_uploaded("a", "1 B", None, scope(), &p);
    assert!(out.contains('\x1b'));
}
