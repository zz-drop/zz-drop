//! State-aware contextual suggestions (SACS).
//!
//! Four pieces of public surface live under tooling subcommands
//! intercepted by `main.rs` before the grammar parser runs:
//!
//! - `zz --help` / `zz -h` — static cheat sheet rendered by [`help`].
//! - `zz --version` / `zz -V` — print `zz-drop <version>` and exit.
//! - `zz --completions <shell>` — emits a shell completion script.
//! - `zz __complete <args>` — hidden subcommand the shell scripts
//!   call on every TAB to fetch contextual candidates as NDJSON.
//!
//! Keeping these out of the [`crate::cli::Command`] grammar
//! preserves the parser's "treat unknown as path" invariant: a
//! user file literally named `--help` or `__complete` still
//! uploads via `zz ./--help` exactly like it did with `q`, `sa`,
//! and other reserved tokens.

pub mod agent_source;
pub mod complete;
pub mod help;
pub mod render;
pub mod scripts;
pub mod state;

use crate::commands::{EXIT_OK, EXIT_USAGE};
use crate::config;
use crate::output;
use agent_source::AgentBridge;

/// Result of [`intercept`]: either we matched a tooling subcommand
/// and returned its exit code, or the args belong to the grammar
/// parser.
pub enum Intercepted {
    Handled(i32),
    PassThrough,
}

/// Dispatch the three tooling subcommands. Match the first arg
/// exactly — never via prefix — so a leading positional like
/// `--foo` (any other dashed token a user might type) falls
/// through to the parser unchanged.
pub fn intercept(args: &[String]) -> Intercepted {
    let head = match args.first() {
        Some(h) => h.as_str(),
        None => return Intercepted::PassThrough,
    };

    match head {
        "--help" | "-h" => {
            print!("{}", help::render(help::detect_columns()));
            Intercepted::Handled(EXIT_OK)
        }
        "--version" | "-V" => {
            println!("zz-drop {}", env!("CARGO_PKG_VERSION"));
            Intercepted::Handled(EXIT_OK)
        }
        "--completions" => Intercepted::Handled(handle_completions(&args[1..])),
        "__complete" => Intercepted::Handled(handle_complete(&args[1..])),
        _ => Intercepted::PassThrough,
    }
}

/// Emit the inlined shell completion script for the requested
/// shell, on stdout. The script is a small, dumb relay: it calls
/// `zz __complete <args>` and formats the NDJSON response.
fn handle_completions(args: &[String]) -> i32 {
    let shell = match args {
        [s] => s.as_str(),
        [] => {
            output::err_line(
                "--completions requires a shell name (bash, zsh, or fish)",
            );
            return EXIT_USAGE;
        }
        _ => {
            output::err_line("--completions takes exactly one argument");
            return EXIT_USAGE;
        }
    };
    let template = match shell {
        "bash" => scripts::BASH,
        "zsh" => scripts::ZSH,
        "fish" => scripts::FISH,
        _ => {
            output::err_line(&format!(
                "--completions: unsupported shell `{shell}` (supported: bash, zsh, fish)"
            ));
            return EXIT_USAGE;
        }
    };
    print!("{template}");
    EXIT_OK
}

/// Run the completion engine: detect state, classify cursor
/// context, emit NDJSON candidates on stdout. Always exits 0 —
/// even when something fails — because the shell calls this on
/// every TAB and any non-zero exit would surface as an error
/// to the operator. Failure modes degrade silently to "no
/// candidates this time".
fn handle_complete(args: &[String]) -> i32 {
    let parsed = complete::parse_args(args);
    let paths = config::discover().ok();
    let mut signals = match &paths {
        Some(p) => state::detect_signals_from_paths(
            &p.profiles_local_file,
            &p.profiles_remote_file,
            &p.agent_socket,
        ),
        // No paths means no container directory found at all —
        // S0 is the right resting state.
        None => state::Signals {
            profiles_local_exists: false,
            profiles_remote_exists: false,
            remote_feature_compiled_in: state::remote_feature_compiled_in(),
            agent_socket_exists: false,
            agent_unlocked: None,
        },
    };

    // Disambiguate S2 vs S3/S4 with one cheap `Status` round-trip
    // when the socket is present. Skipped when there's no socket
    // (S0/S1 stays correct on filesystem signals alone) — saves
    // the cost on every TAB in those states.
    let mut bridge = if signals.agent_socket_exists {
        if let Some(p) = &paths {
            match AgentBridge::probe(&p.agent_socket, &p.token_file) {
                Some(b) => {
                    signals.agent_unlocked = Some(b.unlocked());
                    Some(b)
                }
                None => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    let st = state::classify(&signals);
    let candidates = complete::run(
        st,
        &parsed,
        bridge
            .as_mut()
            .map(|b| b as &mut dyn complete::RemoteListSource),
    );
    print!("{}", render::render(candidates));
    EXIT_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Intercepted {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        intercept(&owned)
    }

    #[test]
    fn empty_args_pass_through() {
        assert!(matches!(run(&[]), Intercepted::PassThrough));
    }

    #[test]
    fn unknown_first_arg_passes_through() {
        // Both a regular path and a flag-shaped non-match must
        // reach the grammar parser unchanged. The parser then
        // decides whether `--foo` is a path or an error.
        assert!(matches!(run(&["readme.md"]), Intercepted::PassThrough));
        assert!(matches!(run(&["--foo"]), Intercepted::PassThrough));
        assert!(matches!(run(&["__not_complete"]), Intercepted::PassThrough));
    }

    #[test]
    fn help_is_intercepted() {
        match run(&["--help"]) {
            Intercepted::Handled(EXIT_OK) => {}
            other => panic!("expected Handled(0), got {:?}", as_i32(other)),
        }
        match run(&["-h"]) {
            Intercepted::Handled(EXIT_OK) => {}
            other => panic!("expected Handled(0), got {:?}", as_i32(other)),
        }
    }

    #[test]
    fn version_is_intercepted() {
        match run(&["--version"]) {
            Intercepted::Handled(EXIT_OK) => {}
            other => panic!("expected Handled(0), got {:?}", as_i32(other)),
        }
        match run(&["-V"]) {
            Intercepted::Handled(EXIT_OK) => {}
            other => panic!("expected Handled(0), got {:?}", as_i32(other)),
        }
    }

    #[test]
    fn completions_requires_shell() {
        match run(&["--completions"]) {
            Intercepted::Handled(EXIT_USAGE) => {}
            other => panic!("expected usage error, got {:?}", as_i32(other)),
        }
        match run(&["--completions", "bash", "extra"]) {
            Intercepted::Handled(EXIT_USAGE) => {}
            other => panic!("expected usage error, got {:?}", as_i32(other)),
        }
    }

    #[test]
    fn completions_rejects_unknown_shell() {
        match run(&["--completions", "powershell"]) {
            Intercepted::Handled(EXIT_USAGE) => {}
            other => panic!("expected usage error, got {:?}", as_i32(other)),
        }
    }

    #[test]
    fn completions_known_shells_succeed() {
        for shell in ["bash", "zsh", "fish"] {
            match run(&["--completions", shell]) {
                Intercepted::Handled(EXIT_OK) => {}
                other => panic!(
                    "shell {shell}: expected ok, got {:?}",
                    as_i32(other)
                ),
            }
        }
    }

    #[test]
    fn complete_stub_is_silent_success() {
        match run(&["__complete"]) {
            Intercepted::Handled(EXIT_OK) => {}
            other => panic!("expected ok, got {:?}", as_i32(other)),
        }
        match run(&["__complete", "d", "rea"]) {
            Intercepted::Handled(EXIT_OK) => {}
            other => panic!("expected ok, got {:?}", as_i32(other)),
        }
    }

    fn as_i32(i: Intercepted) -> Option<i32> {
        match i {
            Intercepted::Handled(c) => Some(c),
            Intercepted::PassThrough => None,
        }
    }
}
