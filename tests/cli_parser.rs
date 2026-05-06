use std::path::PathBuf;

use zz_drop::{Command, ContainerSource, ParseError, parse_args};

fn parse(args: &[&str]) -> Result<Command, ParseError> {
    parse_args(args.iter().map(|s| s.to_string()))
}

#[test]
fn empty_args_returns_no_args_error() {
    let r: [&str; 0] = [];
    assert_eq!(parse(&r), Err(ParseError::NoArgs));
}

#[test]
fn first_non_reserved_arg_is_upload() {
    assert_eq!(
        parse(&["report.pdf"]).unwrap(),
        Command::Upload {
            files: vec![PathBuf::from("report.pdf")],
            compress: false,
            dest_remote: None,
        }
    );
}

#[test]
fn upload_collects_all_args_when_first_is_not_reserved() {
    assert_eq!(
        parse(&["a.txt", "b.txt", "c.txt"]).unwrap(),
        Command::Upload {
            files: vec![
                PathBuf::from("a.txt"),
                PathBuf::from("b.txt"),
                PathBuf::from("c.txt"),
            ],
            compress: false,
            dest_remote: None,
        }
    );
}

#[test]
fn s_is_explicit_upload() {
    assert_eq!(
        parse(&["s", "a.txt", "b.txt"]).unwrap(),
        Command::Upload {
            files: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
            compress: false,
            dest_remote: None,
        }
    );
}

#[test]
fn explicit_path_uploads_file_named_like_command() {
    let cases = ["./ls", "./sa", "./z", "./kalvasflam"];
    for path in cases {
        assert_eq!(
            parse(&[path]).unwrap(),
            Command::Upload {
                files: vec![PathBuf::from(path)],
                compress: false,
                dest_remote: None,
            },
            "explicit-path `{path}` should be uploaded, not interpreted as a command"
        );
    }
}

#[test]
fn save_aggregate_with_dir_only() {
    assert_eq!(
        parse(&["sa", "."]).unwrap(),
        Command::SaveAll {
            compress: false,
            dir: PathBuf::from("."),
            dest_remote: None,
        }
    );
    assert_eq!(
        parse(&["sar", "/tmp/proj"]).unwrap(),
        Command::SaveAllRecursive {
            compress: false,
            dir: PathBuf::from("/tmp/proj"),
            dest_remote: None,
        }
    );
}

#[test]
fn save_aggregate_with_remote_prefix() {
    assert_eq!(
        parse(&["sa", ".", "backup/"]).unwrap(),
        Command::SaveAll {
            compress: false,
            dir: PathBuf::from("."),
            dest_remote: Some("backup".into()),
        }
    );
    assert_eq!(
        parse(&["sar", "./project", "backup/snap/"]).unwrap(),
        Command::SaveAllRecursive {
            compress: false,
            dir: PathBuf::from("./project"),
            dest_remote: Some("backup/snap".into()),
        }
    );
}

#[test]
fn download_aggregate_zero_args_defaults_to_cwd_and_root() {
    assert_eq!(
        parse(&["da"]).unwrap(),
        Command::DownloadAll {
            decompress: false,
            dest_local: None,
            src_remote: None,
        }
    );
    assert_eq!(
        parse(&["dar"]).unwrap(),
        Command::DownloadAllRecursive {
            decompress: false,
            dest_local: None,
            src_remote: None,
        }
    );
}

#[test]
fn download_aggregate_with_local_dest_only() {
    assert_eq!(
        parse(&["da", "backup"]).unwrap(),
        Command::DownloadAll {
            decompress: false,
            dest_local: Some(PathBuf::from("backup")),
            src_remote: None,
        }
    );
    assert_eq!(
        parse(&["dar", "./snapshot"]).unwrap(),
        Command::DownloadAllRecursive {
            decompress: false,
            dest_local: Some(PathBuf::from("./snapshot")),
            src_remote: None,
        }
    );
}

#[test]
fn download_aggregate_with_local_dest_and_remote_prefix() {
    assert_eq!(
        parse(&["da", "backup", "docs/"]).unwrap(),
        Command::DownloadAll {
            decompress: false,
            dest_local: Some(PathBuf::from("backup")),
            src_remote: Some("docs".into()),
        }
    );
    assert_eq!(
        parse(&["dar", "./snapshot", "project/build/"]).unwrap(),
        Command::DownloadAllRecursive {
            decompress: false,
            dest_local: Some(PathBuf::from("./snapshot")),
            src_remote: Some("project/build".into()),
        }
    );
}

#[test]
fn upload_multi_with_remote_dir_dest() {
    assert_eq!(
        parse(&["s", "a.md", "b.md", "docs/"]).unwrap(),
        Command::Upload {
            files: vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
            compress: false,
            dest_remote: Some("docs".into()),
        }
    );
}

#[test]
fn download_multi_with_local_dir_dest() {
    assert_eq!(
        parse(&["d", "api.md", "guide.md", "./out/"]).unwrap(),
        Command::Download {
            files: vec!["api.md".into(), "guide.md".into()],
            decompress: false,
            dest_local: Some(PathBuf::from("./out")),
        }
    );
}

#[test]
fn upload_with_only_trailing_slash_arg_is_missing_source() {
    // `zz s docs/` — no source files, the trailing-`/` arg is
    // by definition the destination.
    assert_eq!(
        parse(&["s", "docs/"]),
        Err(ParseError::MissingArgument { command: "s" })
    );
    assert_eq!(
        parse(&["d", "out/"]),
        Err(ParseError::MissingArgument { command: "d" })
    );
}

#[test]
fn lock_wipe_and_kalvasflam() {
    assert_eq!(parse(&["q"]).unwrap(), Command::Lock);
    assert_eq!(parse(&["w"]).unwrap(), Command::Wipe);
    assert_eq!(parse(&["kalvasflam"]).unwrap(), Command::Wipe);
}

#[test]
fn x_is_no_longer_a_command() {
    assert_eq!(
        parse(&["x"]).unwrap(),
        Command::Upload {
            files: vec![PathBuf::from("x")],
            compress: false,
            dest_remote: None,
        },
    );
}

#[test]
fn z_no_args_picks_default_container() {
    assert_eq!(
        parse(&["z"]).unwrap(),
        Command::ContainerUnlock { which: None },
    );
}

#[test]
fn z_local_and_remote_pick_explicitly() {
    assert_eq!(
        parse(&["z", "local"]).unwrap(),
        Command::ContainerUnlock {
            which: Some(ContainerSource::Local)
        },
    );
    assert_eq!(
        parse(&["z", "remote"]).unwrap(),
        Command::ContainerUnlock {
            which: Some(ContainerSource::Remote)
        },
    );
}

#[test]
fn z_extra_arguments_are_an_error() {
    // 1-arg forms now route either to ContainerUnlock (local/remote)
    // or to RemoteUnlock (email/alias). What's still rejected is
    // 2+ arguments — see TASK 20 (CLI client).
    assert_eq!(
        parse(&["z", "local", "extra"]),
        Err(ParseError::UnexpectedArgument { command: "z" })
    );
    assert_eq!(
        parse(&["z", "casa-nc", "extra"]),
        Err(ParseError::UnexpectedArgument { command: "z" })
    );
}

#[test]
fn open_tui_and_doctor_have_no_args() {
    assert_eq!(parse(&["c"]).unwrap(), Command::OpenTui);
    assert_eq!(parse(&["f"]).unwrap(), Command::Doctor);
}

#[test]
fn missing_required_args() {
    assert_eq!(
        parse(&["s"]),
        Err(ParseError::MissingArgument { command: "s" })
    );
    assert_eq!(
        parse(&["d"]),
        Err(ParseError::MissingArgument { command: "d" })
    );
    // Save-all variants still require the local source dir.
    assert_eq!(
        parse(&["sa"]),
        Err(ParseError::MissingArgument { command: "sa" })
    );
    assert_eq!(
        parse(&["sar"]),
        Err(ParseError::MissingArgument { command: "sar" })
    );
}

#[test]
fn unexpected_extra_args() {
    // Atom commands that take no positional args at all.
    for cmd in ["q", "w", "kalvasflam", "c", "f"] {
        let res = parse(&[cmd, "junk"]);
        assert_eq!(
            res,
            Err(ParseError::UnexpectedArgument { command: leak_static(cmd) }),
            "command `{cmd}` should reject extra args"
        );
    }
    // `sa`/`sar`/`da`/`dar` accept up to two positionals
    // (local then optional remote prefix). A third is rejected.
    for cmd in ["sa", "sar", "da", "dar"] {
        let res = parse(&[cmd, ".", "remote/", "extra"]);
        assert_eq!(
            res,
            Err(ParseError::UnexpectedArgument { command: leak_static(cmd) }),
            "command `{cmd} . remote/ extra` should reject the third positional"
        );
    }

    assert_eq!(
        parse(&["z", "local", "extra"]),
        Err(ParseError::UnexpectedArgument { command: "z" })
    );
}

// Map to the `&'static str` ParseError carries. The dispatch
// always uses the string literal that matches the command verbatim.
fn leak_static(s: &str) -> &'static str {
    match s {
        "s" => "s",
        "sa" => "sa",
        "sar" => "sar",
        "d" => "d",
        "da" => "da",
        "dar" => "dar",
        "q" => "q",
        "w" => "w",
        "kalvasflam" => "kalvasflam",
        "z" => "z",
        "c" => "c",
        "f" => "f",
        _ => panic!("unexpected command in test: {s}"),
    }
}

#[test]
fn parse_error_display_messages_are_helpful() {
    let err = parse_args::<_, String>([]).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("no arguments"), "got `{s}`");

    let err = parse(&["s"]).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("`s`") && s.contains("requires"), "got `{s}`");

    let err = parse(&["z", "local", "junk"]).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("`z`") && s.contains("no arguments"), "got `{s}`");
}
