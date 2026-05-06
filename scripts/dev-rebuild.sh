#!/usr/bin/env bash
# Coordinated rebuild of the three zz-drop crates.
#
# zz-drop, zz-drop-tui and zz-drop-core live in separate cargo
# workspaces (no top-level `Cargo.toml`). Rebuilding one does NOT
# rebuild the others, which is how a postcard wire-format change
# in `zz-drop-core` can land in `zz-drop` while `zz-drop-tui`
# keeps the old layout — agent zombies, frame truncation, the
# whole circus.
#
# Run this script after any change that touches `zz-drop-core`
# (and ideally on every dev session). It:
#
#   1. Kills any running zz-drop agent so the next `zz z`
#      respawns one from the freshly-built binary.
#   2. Builds zz-drop-core (release).
#   3. Builds zz-drop (release) — depends on core.
#   4. Builds zz-drop-tui (release) — depends on core.
#
# Pass `--profile dev` to use unoptimised builds for fast iteration.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORE="$PROJECT_ROOT/zz-drop-core"
CLI="$PROJECT_ROOT/zz-drop"
TUI="$PROJECT_ROOT/zz-drop-tui"

PROFILE="release"
PROFILE_FLAG="--release"
if [[ "${1:-}" == "--profile" && "${2:-}" == "dev" ]]; then
    PROFILE="debug"
    PROFILE_FLAG=""
fi

for d in "$CORE" "$CLI" "$TUI"; do
    if [[ ! -d "$d" ]]; then
        echo "error: missing crate dir: $d" >&2
        exit 1
    fi
done

echo "[1/4] killing any running zz-drop agent…"
# The agent identifies itself with the env var
# `ZZ_DROP_AGENT_MODE=1`, set in its environment block. `pkill -f`
# only matches the process command line, not env vars, so it
# cannot find the agent on macOS. Use `ps -E` (BSD ps) which
# dumps the full environment per process; on Linux fall back to
# /proc/<pid>/environ.
killed=0
# macOS / BSD: `ps -E` dumps the environment in the COMMAND
# column — but ONLY when no `-o` format is given. With `-o`
# the env block is silently dropped, so the agent (which is
# only identifiable by its env var) is invisible. Use the
# default columns and let awk pull the leading pid.
#
# We probe by running the pipeline directly: if `ps -E -ax`
# returns nothing useful (e.g. on Linux), the var stays empty
# and we fall through to the /proc branch. We can't guard with
# `ps -E -ax | grep -q .` because under `pipefail`, SIGPIPE
# (141) on the upstream `ps` makes the conditional false even
# when output exists.
pids=$(ps -E -ax 2>/dev/null \
        | grep ZZ_DROP_AGENT_MODE \
        | grep -v grep \
        | awk '{print $1}' || true)
if [[ -n "$pids" ]]; then
    for pid in $pids; do
        if kill -TERM "$pid" 2>/dev/null; then
            killed=$((killed + 1))
            echo "  SIGTERM → pid $pid"
        fi
    done
elif [[ -d /proc ]]; then
    # Linux: walk /proc and check environ for the marker.
    for env in /proc/*/environ; do
        pid="${env#/proc/}"
        pid="${pid%/environ}"
        if [[ "$pid" =~ ^[0-9]+$ ]] \
            && tr '\0' '\n' < "$env" 2>/dev/null \
                | grep -q '^ZZ_DROP_AGENT_MODE='
        then
            if kill -TERM "$pid" 2>/dev/null; then
                killed=$((killed + 1))
                echo "  SIGTERM → pid $pid"
            fi
        fi
    done
fi

# Also clean up the runtime dir state left by the killed agent
# (socket + token + lock). The new agent spawn relies on the
# socket being absent.
runtime_dir="${XDG_RUNTIME_DIR:-/tmp/zz-drop-$(id -u)}"
[[ -d "$runtime_dir" ]] || runtime_dir="/tmp/zz-drop-$(id -u)"
if [[ -d "$runtime_dir" ]]; then
    rm -f "$runtime_dir/agent.sock" "$runtime_dir/token" "$runtime_dir/agent.lock"
fi

if (( killed == 0 )); then
    echo "  no agent running"
fi

echo "[2/4] building zz-drop-core ($PROFILE)…"
( cd "$CORE" && cargo build $PROFILE_FLAG )

echo "[3/4] building zz-drop ($PROFILE)…"
( cd "$CLI" && cargo build $PROFILE_FLAG )

echo "[4/4] building zz-drop-tui ($PROFILE)…"
( cd "$TUI" && cargo build $PROFILE_FLAG )

echo
echo "done. release binaries:"
echo "  $CLI/target/$PROFILE/zz-drop"
echo "  $TUI/target/$PROFILE/zz-tui"
echo
echo "to use them in your shell:"
echo "  export PATH=\"$CLI/target/$PROFILE:$TUI/target/$PROFILE:\$PATH\""
