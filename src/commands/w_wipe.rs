use std::io::{IsTerminal, Write};

use zz_drop_core::config::Paths;
use zz_drop_core::scriptable::Reason;

use crate::agent::AgentClient;
use crate::commands::{EXIT_OK, EXIT_USAGE, EXIT_WIPE_CANCELLED};
use crate::output;
use crate::runtime::{self, OutputMode};

pub fn run(paths: &Paths) -> i32 {
    let mode = runtime::flags().output;
    let yes = runtime::flags().yes;

    // Scriptable modes must never prompt. Require an explicit
    // `--yes` (or the legacy `ZZ_DROP_CONFIRM_WIPE=yes` env, kept
    // for compat) to proceed; otherwise emit `interactive_required`.
    if matches!(mode, OutputMode::Json | OutputMode::Quiet) && !yes && !legacy_env_yes() {
        output::emit_failed_bare(
            Reason::InteractiveRequired,
            Some("`zz w` is destructive — pass `--yes` to confirm in scriptable mode"),
        );
        return EXIT_USAGE;
    }

    if !confirm_wipe(yes) {
        output::emit_failed_bare(Reason::WipeCancelled, None);
        return EXIT_WIPE_CANCELLED;
    }

    // best-effort agent shutdown
    if paths.agent_socket.exists()
        && let Ok(mut client) = AgentClient::connect(&paths.agent_socket, &paths.token_file)
    {
        let _ = client.exit();
    }

    let mut errors: Vec<String> = Vec::new();

    let _ = std::fs::remove_file(&paths.profiles_local_file);
    let _ = std::fs::remove_file(&paths.profiles_remote_file);
    let _ = std::fs::remove_file(&paths.config_file);
    let _ = std::fs::remove_file(&paths.agent_socket);
    let _ = std::fs::remove_file(&paths.token_file);

    // remove runtime dir entirely (may contain other zz-drop artifacts)
    if let Err(e) = remove_dir_if_exists(&paths.runtime_dir) {
        errors.push(format!("runtime: {e}"));
    }
    // remove config dir if empty
    let _ = std::fs::remove_dir(&paths.config_dir);

    if errors.is_empty() {
        output::emit_wiped();
        EXIT_OK
    } else {
        for e in &errors {
            output::emit_failed_bare(Reason::Usage, Some(e));
        }
        EXIT_USAGE
    }
}

fn legacy_env_yes() -> bool {
    std::env::var("ZZ_DROP_CONFIRM_WIPE").as_deref() == Ok("yes")
}

fn confirm_wipe(yes_flag: bool) -> bool {
    if yes_flag || legacy_env_yes() {
        return true;
    }
    if std::io::stdin().is_terminal() {
        eprint!("type \"wipe\" to confirm: ");
        let _ = std::io::stderr().flush();
        let mut buf = String::new();
        if std::io::stdin().read_line(&mut buf).is_err() {
            return false;
        }
        buf.trim() == "wipe"
    } else {
        false
    }
}

fn remove_dir_if_exists(p: &std::path::Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
