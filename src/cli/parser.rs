use std::fmt;
use std::path::PathBuf;

use super::{Command, ContainerSource, RemoteSelector};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    NoArgs,
    MissingArgument { command: &'static str },
    UnexpectedArgument { command: &'static str },
    /// The token started with `s` or `d` and looked like a
    /// composite command, but a letter outside the v1 modifier
    /// alphabet `{a, r, x}` showed up.
    UnknownModifier { command: String, modifier: char },
    /// The composite token used the `e` modifier. Recognised on
    /// purpose so the operator gets a clear "v1.1" message
    /// instead of falling through to "unknown command" or being
    /// silently treated as a path.
    EncryptNotInV1 { command: String },
    /// A modifier letter appeared twice in the same token
    /// (e.g. `saa`). Set semantics — at most one occurrence.
    DuplicateModifier { command: String, modifier: char },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoArgs => f.write_str(
                "no arguments; run `zz <file>` to upload, or `zz f` for diagnostics",
            ),
            Self::MissingArgument { command } => {
                write!(f, "`{command}` requires at least one argument")
            }
            Self::UnexpectedArgument { command } => {
                write!(f, "`{command}` takes no arguments")
            }
            Self::UnknownModifier { command, modifier } => {
                write!(
                    f,
                    "`{command}`: unknown modifier `{modifier}` (valid for v1: a, r, x)"
                )
            }
            Self::EncryptNotInV1 { command } => {
                write!(
                    f,
                    "`{command}`: encryption (`e`) is coming in v1.1; v1 supports `a`, `r`, `x`"
                )
            }
            Self::DuplicateModifier { command, modifier } => {
                write!(
                    f,
                    "`{command}`: modifier `{modifier}` repeated (set semantics — at most once)"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Atom commands — non-composite verbs. The composite verbs `s`
/// and `d` accept a modifier suffix and are dispatched by
/// `parse_composite_modifiers` below.
const ATOM_RESERVED: &[&str] = &["q", "w", "kalvasflam", "z", "c", "f"];

/// Subset of letters the v1 grammar accepts after `s` / `d`.
/// `e` is recognised separately (with a v1.1 message) and `a`,
/// `r`, `x` carry semantics:
/// - `a` — apply to all files in cwd (or `da` for download).
/// - `r` — recurse into subdirectories.
/// - `x` — compress on upload / decompress on download (zstd).
const VALID_MODIFIERS: &[char] = &['a', 'r', 'x'];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Modifiers {
    a: bool,
    r: bool,
    x: bool,
}

/// Parse the suffix of a composite token (`sx`, `sax`, `darx`,
/// etc.) into a [`Modifiers`] set. The verb byte (`s` or `d`)
/// is *not* part of `tail`.
fn parse_modifiers(verb: char, tail: &str) -> Result<Modifiers, ParseError> {
    let mut m = Modifiers::default();
    for c in tail.chars() {
        match c {
            'e' => {
                return Err(ParseError::EncryptNotInV1 {
                    command: format!("{verb}{tail}"),
                });
            }
            'a' if !m.a => m.a = true,
            'r' if !m.r => m.r = true,
            'x' if !m.x => m.x = true,
            'a' | 'r' | 'x' => {
                return Err(ParseError::DuplicateModifier {
                    command: format!("{verb}{tail}"),
                    modifier: c,
                });
            }
            other => {
                return Err(ParseError::UnknownModifier {
                    command: format!("{verb}{tail}"),
                    modifier: other,
                });
            }
        }
    }
    debug_assert!(VALID_MODIFIERS.contains(&'a'));
    Ok(m)
}

fn is_reserved(arg: &str) -> bool {
    if ATOM_RESERVED.contains(&arg) {
        return true;
    }
    let mut chars = arg.chars();
    match chars.next() {
        Some('s') | Some('d') => {
            // Tail must be a permutation of letters from the
            // accepted modifier alphabet (or `e`, which we want
            // to surface specifically). Anything else falls
            // through to "treat as filename".
            let tail = &arg[1..];
            tail.chars()
                .all(|c| VALID_MODIFIERS.contains(&c) || c == 'e')
        }
        _ => false,
    }
}

pub fn parse_args<I, S>(args: I) -> Result<Command, ParseError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let head = args.first().ok_or(ParseError::NoArgs)?;
    let tail = &args[1..];

    if !is_reserved(head) {
        // Default verb (no token): every arg is an upload source.
        // No trailing-`/` magic here — the whole point of the
        // composite forms is to disambiguate destinations.
        let files = args.iter().map(PathBuf::from).collect();
        return Ok(Command::Upload {
            files,
            compress: false,
            dest_remote: None,
        });
    }

    // Composite verbs `s` and `d` parse their suffix as a
    // modifier set. `s` requires file args unless `a` is set;
    // `d` requires file args unless `a` is set; `a` + `r`
    // means "recurse"; `x` means "compress on upload /
    // decompress on download".
    if let Some(rest) = head.strip_prefix('s') {
        let m = parse_modifiers('s', rest)?;
        return if m.a {
            // `sa <dir> [<remote-prefix>]`. 1st arg is the local
            // source dir (typically `.`); the optional 2nd arg
            // is a remote sub-prefix relative to `<remote_root>`.
            let label = if m.r { "sar" } else { "sa" };
            let (dir, dest_remote) = require_local_then_optional_remote(label, tail)?;
            Ok(if m.r {
                Command::SaveAllRecursive {
                    compress: m.x,
                    dir: PathBuf::from(dir),
                    dest_remote,
                }
            } else {
                Command::SaveAll {
                    compress: m.x,
                    dir: PathBuf::from(dir),
                    dest_remote,
                }
            })
        } else if m.r {
            // `r` only makes sense paired with `a` — `sr` /
            // `srx` aren't valid forms in v1. Reject explicitly.
            Err(ParseError::UnknownModifier {
                command: head.to_string(),
                modifier: 'r',
            })
        } else if tail.is_empty() {
            Err(ParseError::MissingArgument { command: "s" })
        } else {
            // Multi-source upload: if the LAST positional ends
            // with `/`, it is the remote destination directory
            // and the rest are sources. Without trailing `/`
            // every positional is a source.
            let (sources, dest_remote) = split_trailing_dir_dest("s", tail)?;
            Ok(Command::Upload {
                files: sources.iter().map(PathBuf::from).collect(),
                compress: m.x,
                dest_remote,
            })
        };
    }
    if let Some(rest) = head.strip_prefix('d') {
        let m = parse_modifiers('d', rest)?;
        return if m.a {
            // `da [<local-dest> [<remote-prefix>]]`. Both args
            // optional. 1st = local dest dir (default cwd);
            // 2nd = remote sub-prefix (default = root).
            let label = if m.r { "dar" } else { "da" };
            let (dest_local, src_remote) = parse_optional_local_remote(label, tail)?;
            Ok(if m.r {
                Command::DownloadAllRecursive {
                    decompress: m.x,
                    dest_local,
                    src_remote,
                }
            } else {
                Command::DownloadAll {
                    decompress: m.x,
                    dest_local,
                    src_remote,
                }
            })
        } else if m.r {
            Err(ParseError::UnknownModifier {
                command: head.to_string(),
                modifier: 'r',
            })
        } else if tail.is_empty() {
            Err(ParseError::MissingArgument { command: "d" })
        } else {
            // Multi-source download: same trailing-`/` rule as
            // `s` — last arg ending in `/` is the local dest
            // directory; all earlier args are remote names.
            let (sources, dest_remote) = split_trailing_dir_dest("d", tail)?;
            // For `d`, the "trailing-dir" arg is local, not
            // remote — reuse the helper but rename here.
            let dest_local = dest_remote.map(PathBuf::from);
            Ok(Command::Download {
                files: sources.iter().map(|s| s.to_string()).collect(),
                decompress: m.x,
                dest_local,
            })
        };
    }

    match head.as_str() {
        "q" => {
            require_no_args("q", tail)?;
            Ok(Command::Lock)
        }
        "w" => {
            require_no_args("w", tail)?;
            Ok(Command::Wipe)
        }
        "kalvasflam" => {
            require_no_args("kalvasflam", tail)?;
            Ok(Command::Wipe)
        }
        "z" => match tail.len() {
            0 => Ok(Command::ContainerUnlock { which: None }),
            1 => match tail[0].as_str() {
                "local" => Ok(Command::ContainerUnlock {
                    which: Some(ContainerSource::Local),
                }),
                "remote" => Ok(Command::ContainerUnlock {
                    which: Some(ContainerSource::Remote),
                }),
                // Anything else with one `@` is interpreted as an
                // account email; otherwise as a stored alias. The
                // executor decides whether the runtime supports it
                // (gated behind the `remote` Cargo feature).
                other if other.contains('@') => Ok(Command::RemoteUnlock {
                    selector: RemoteSelector::Email(other.to_string()),
                }),
                other if !other.is_empty() => Ok(Command::RemoteUnlock {
                    selector: RemoteSelector::Alias(other.to_string()),
                }),
                _ => Err(ParseError::UnexpectedArgument { command: "z" }),
            },
            _ => Err(ParseError::UnexpectedArgument { command: "z" }),
        },
        "c" => {
            require_no_args("c", tail)?;
            Ok(Command::OpenTui)
        }
        "f" => {
            require_no_args("f", tail)?;
            Ok(Command::Doctor)
        }
        _ => unreachable!("ATOM_RESERVED must match this dispatch"),
    }
}

fn require_no_args(command: &'static str, tail: &[String]) -> Result<(), ParseError> {
    if tail.is_empty() {
        Ok(())
    } else {
        Err(ParseError::UnexpectedArgument { command })
    }
}

/// `sa` / `sar` / `sax` / `sarx` take a required local dir
/// argument and an optional remote-prefix as the second
/// positional. Trailing `/` on either is normalised away.
fn require_local_then_optional_remote(
    command: &'static str,
    tail: &[String],
) -> Result<(String, Option<String>), ParseError> {
    match tail.len() {
        0 => Err(ParseError::MissingArgument { command }),
        1 => Ok((normalise_local(&tail[0]), None)),
        2 => Ok((
            normalise_local(&tail[0]),
            Some(normalise_remote_prefix(&tail[1])),
        )),
        _ => Err(ParseError::UnexpectedArgument { command }),
    }
}

/// `da` / `dar` / `dax` / `darx` accept zero, one, or two
/// positionals: `[<local-dest> [<remote-prefix>]]`. Trailing
/// `/` on either is normalised away.
fn parse_optional_local_remote(
    command: &'static str,
    tail: &[String],
) -> Result<(Option<PathBuf>, Option<String>), ParseError> {
    match tail.len() {
        0 => Ok((None, None)),
        1 => Ok((Some(PathBuf::from(normalise_local(&tail[0]))), None)),
        2 => Ok((
            Some(PathBuf::from(normalise_local(&tail[0]))),
            Some(normalise_remote_prefix(&tail[1])),
        )),
        _ => Err(ParseError::UnexpectedArgument { command }),
    }
}

/// Multi-source `s` / `d`: if the LAST arg ends with `/`, it is
/// the destination directory and the rest are sources;
/// otherwise every arg is a source.
///
/// Returns `(sources, dest_dir_normalised)`. Errors when the
/// trailing-`/` arg is the only arg (no source files).
fn split_trailing_dir_dest<'a>(
    command: &'static str,
    tail: &'a [String],
) -> Result<(Vec<&'a str>, Option<String>), ParseError> {
    if tail.is_empty() {
        return Err(ParseError::MissingArgument { command });
    }
    let last = &tail[tail.len() - 1];
    if last.ends_with('/') {
        if tail.len() == 1 {
            // `zz s docs/` or `zz d docs/` — the trailing-`/`
            // arg by itself is a destination with no sources.
            return Err(ParseError::MissingArgument { command });
        }
        let sources: Vec<&'a str> = tail[..tail.len() - 1].iter().map(String::as_str).collect();
        Ok((sources, Some(strip_trailing_slash(last).to_string())))
    } else {
        Ok((tail.iter().map(String::as_str).collect(), None))
    }
}

/// Strip a single trailing `/` if present. Used to normalise
/// trailing-slash hints into the same canonical form whether or
/// not the operator typed it.
fn strip_trailing_slash(s: &str) -> &str {
    s.strip_suffix('/').unwrap_or(s)
}

/// Local paths can carry a trailing `/` (`./out/`, `~/foo/`)
/// without changing meaning. Strip it so downstream code holds
/// the canonical form.
fn normalise_local(s: &str) -> String {
    strip_trailing_slash(s).to_string()
}

/// Remote prefixes are always cloud-side relative paths. Strip
/// trailing `/` for normalisation. The leading `/` is also
/// stripped — remote prefixes are relative to `<remote_root>`
/// and a leading `/` would otherwise reach into ambiguity.
fn normalise_remote_prefix(s: &str) -> String {
    let trimmed = s.trim_matches('/');
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, ParseError> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_args_is_error() {
        let r: [&str; 0] = [];
        assert_eq!(parse(&r), Err(ParseError::NoArgs));
    }

    #[test]
    fn upload_is_default() {
        assert_eq!(
            parse(&["readme.md"]).unwrap(),
            Command::Upload {
                files: vec![PathBuf::from("readme.md")],
                compress: false,
                dest_remote: None,
            }
        );
    }

    #[test]
    fn upload_multiple_files() {
        assert_eq!(
            parse(&["a.md", "b.md", "c.md"]).unwrap(),
            Command::Upload {
                files: vec![
                    PathBuf::from("a.md"),
                    PathBuf::from("b.md"),
                    PathBuf::from("c.md"),
                ],
                compress: false,
                dest_remote: None,
            }
        );
    }

    #[test]
    fn explicit_upload_with_s() {
        assert_eq!(
            parse(&["s", "readme.md"]).unwrap(),
            Command::Upload {
                files: vec![PathBuf::from("readme.md")],
                compress: false,
                dest_remote: None,
            }
        );
    }

    #[test]
    fn save_all_variants_require_local_dir() {
        assert_eq!(
            parse(&["sa", "."]).unwrap(),
            Command::SaveAll {
                compress: false,
                dir: PathBuf::from("."),
                dest_remote: None,
            }
        );
        assert_eq!(
            parse(&["sar", "/tmp/snapshot"]).unwrap(),
            Command::SaveAllRecursive {
                compress: false,
                dir: PathBuf::from("/tmp/snapshot"),
                dest_remote: None,
            }
        );
        // Missing arg → error.
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
    fn save_all_with_explicit_remote_prefix() {
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
    fn download_aggregate_variants_accept_zero_to_two_args() {
        // Zero args: cwd local, root remote.
        assert_eq!(
            parse(&["da"]).unwrap(),
            Command::DownloadAll {
                decompress: false,
                dest_local: None,
                src_remote: None,
            }
        );
        // 1 arg: local dest only.
        assert_eq!(
            parse(&["da", "snapshot"]).unwrap(),
            Command::DownloadAll {
                decompress: false,
                dest_local: Some(PathBuf::from("snapshot")),
                src_remote: None,
            }
        );
        // 2 args: local dest + remote prefix.
        assert_eq!(
            parse(&["dar", "snapshot", "docs/"]).unwrap(),
            Command::DownloadAllRecursive {
                decompress: false,
                dest_local: Some(PathBuf::from("snapshot")),
                src_remote: Some("docs".into()),
            }
        );
    }

    #[test]
    fn download_single_file_default_dest_is_cwd() {
        assert_eq!(
            parse(&["d", "remote.md"]).unwrap(),
            Command::Download {
                files: vec!["remote.md".into()],
                decompress: false,
                dest_local: None,
            }
        );
    }

    #[test]
    fn x_modifier_sets_compress_on_upload() {
        assert_eq!(
            parse(&["sx", "readme.md"]).unwrap(),
            Command::Upload {
                files: vec![PathBuf::from("readme.md")],
                compress: true,
                dest_remote: None,
            }
        );
    }

    #[test]
    fn x_modifier_sets_decompress_on_download() {
        assert_eq!(
            parse(&["dx", "readme.md.zst"]).unwrap(),
            Command::Download {
                files: vec!["readme.md.zst".into()],
                decompress: true,
                dest_local: None,
            }
        );
    }

    #[test]
    fn save_all_with_compress_modifier_in_any_order() {
        let canonical = Command::SaveAll {
            compress: true,
            dir: PathBuf::from("."),
            dest_remote: None,
        };
        assert_eq!(parse(&["sax", "."]).unwrap(), canonical, "sax");
        assert_eq!(parse(&["sxa", "."]).unwrap(), canonical, "sxa");
    }

    #[test]
    fn save_all_recursive_with_compress_in_any_order() {
        let canonical = Command::SaveAllRecursive {
            compress: true,
            dir: PathBuf::from("."),
            dest_remote: None,
        };
        assert_eq!(parse(&["sarx", "."]).unwrap(), canonical, "sarx");
        assert_eq!(parse(&["sxar", "."]).unwrap(), canonical, "sxar");
        assert_eq!(parse(&["sxra", "."]).unwrap(), canonical, "sxra");
    }

    #[test]
    fn sar_equals_sra_set_semantics_no_compress() {
        let canonical = Command::SaveAllRecursive {
            compress: false,
            dir: PathBuf::from("."),
            dest_remote: None,
        };
        assert_eq!(parse(&["sar", "."]).unwrap(), canonical, "sar");
        assert_eq!(parse(&["sra", "."]).unwrap(), canonical, "sra");
    }

    #[test]
    fn dar_equals_dra_set_semantics_no_decompress() {
        let canonical = Command::DownloadAllRecursive {
            decompress: false,
            dest_local: Some(PathBuf::from(".")),
            src_remote: None,
        };
        assert_eq!(parse(&["dar", "."]).unwrap(), canonical, "dar");
        assert_eq!(parse(&["dra", "."]).unwrap(), canonical, "dra");
    }

    #[test]
    fn download_all_with_decompress_in_any_order() {
        assert_eq!(
            parse(&["dax", "."]).unwrap(),
            Command::DownloadAll {
                decompress: true,
                dest_local: Some(PathBuf::from(".")),
                src_remote: None,
            }
        );
        assert_eq!(
            parse(&["darx", "snapshot"]).unwrap(),
            Command::DownloadAllRecursive {
                decompress: true,
                dest_local: Some(PathBuf::from("snapshot")),
                src_remote: None,
            }
        );
    }

    #[test]
    fn upload_multi_with_trailing_slash_dest_dir() {
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
    fn download_multi_with_trailing_slash_dest_dir() {
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
    fn s_or_d_with_only_trailing_slash_arg_is_missing_source() {
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
    fn encrypt_modifier_is_explicitly_v1_1() {
        // Don't fall through to "unknown command" or upload —
        // the operator typed `e` because they read the README,
        // give them a useful pointer.
        let err = parse(&["sex"]).unwrap_err();
        match err {
            ParseError::EncryptNotInV1 { command } => {
                assert_eq!(command, "sex");
            }
            other => panic!("expected EncryptNotInV1, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_modifier_is_rejected() {
        let err = parse(&["saa"]).unwrap_err();
        assert!(
            matches!(err, ParseError::DuplicateModifier { modifier: 'a', .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn r_without_a_is_unknown_modifier() {
        // `sr` / `srx` aren't valid v1 shapes — `r` only
        // adds recursion to an `a` form.
        let err = parse(&["sr"]).unwrap_err();
        assert!(matches!(err, ParseError::UnknownModifier { modifier: 'r', .. }));
        let err = parse(&["dr", "x"]).unwrap_err();
        assert!(matches!(err, ParseError::UnknownModifier { modifier: 'r', .. }));
    }

    #[test]
    fn unrecognised_modifier_letter_falls_through_to_upload() {
        // `sz` isn't a composite (`z` not in modifier set);
        // it's a path argument, parsed as upload.
        assert_eq!(
            parse(&["sz"]).unwrap(),
            Command::Upload {
                files: vec![PathBuf::from("sz")],
                compress: false,
                dest_remote: None,
            }
        );
    }

    #[test]
    fn lock_wipe_open_doctor() {
        assert_eq!(parse(&["q"]).unwrap(), Command::Lock);
        assert_eq!(parse(&["w"]).unwrap(), Command::Wipe);
        assert_eq!(parse(&["kalvasflam"]).unwrap(), Command::Wipe);
        assert_eq!(parse(&["c"]).unwrap(), Command::OpenTui);
        assert_eq!(parse(&["f"]).unwrap(), Command::Doctor);
    }

    #[test]
    fn z_no_args_is_default_container_unlock() {
        assert_eq!(
            parse(&["z"]).unwrap(),
            Command::ContainerUnlock { which: None },
        );
    }

    #[test]
    fn z_local_and_remote_pick_the_container() {
        assert_eq!(
            parse(&["z", "local"]).unwrap(),
            Command::ContainerUnlock {
                which: Some(ContainerSource::Local),
            },
        );
        assert_eq!(
            parse(&["z", "remote"]).unwrap(),
            Command::ContainerUnlock {
                which: Some(ContainerSource::Remote),
            },
        );
    }

    #[test]
    fn z_with_email_or_alias_yields_remote_unlock() {
        // Anything containing `@` is parsed as an account email;
        // anything else as a stored alias. Executor (gated behind
        // the `remote` feature) decides what to do at runtime.
        assert_eq!(
            parse(&["z", "alice@example.org"]),
            Ok(Command::RemoteUnlock {
                selector: RemoteSelector::Email("alice@example.org".into()),
            }),
        );
        assert_eq!(
            parse(&["z", "personal"]),
            Ok(Command::RemoteUnlock {
                selector: RemoteSelector::Alias("personal".into()),
            }),
        );
    }

    #[test]
    fn z_with_too_many_args_is_error() {
        assert_eq!(
            parse(&["z", "local", "extra"]),
            Err(ParseError::UnexpectedArgument { command: "z" }),
        );
    }

    #[test]
    fn x_is_no_longer_reserved_and_uploads_a_file_named_x() {
        // `zz x` used to mean "unlock". The container model
        // removed it; what was a verb is now just a regular
        // path argument. The `x` modifier only carries meaning
        // *after* `s` or `d`.
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
    fn dotted_path_for_reserved_name_is_upload() {
        for name in ["./ls", "./sa", "./z", "./kalvasflam"] {
            assert_eq!(
                parse(&[name]).unwrap(),
                Command::Upload {
                    files: vec![PathBuf::from(name)],
                    compress: false,
                    dest_remote: None,
                },
                "{name} should be parsed as upload via explicit path"
            );
        }
    }
}
