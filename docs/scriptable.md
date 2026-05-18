# Scriptable mode (`--json` / `--quiet`)

`zz-drop` ships two output contracts beyond the default
human-friendly text:

- **`--json`** — newline-delimited JSON (NDJSON) on stdout, one
  record per result. Schema `v: "1"`, stable from 1.0.0 onward,
  documented below and constrained by the JSON Schema file at
  [`docs/scriptable/zz-drop-output.v1.json`](scriptable/zz-drop-output.v1.json).
- **`--quiet`** — single minimal text line per result. No ANSI
  color, no banners. Useful when a human runs `zz` from a script
  and wants compact terminal output without parsing JSON.

`--quiet` and `--json` are mutually exclusive: passing both
errors out with exit code `2` before any output is produced.

This page is the contract. If anything here disagrees with the
binary, the binary is a bug.

---

## Why a scriptable mode

The default text output exists to give a fast operator-friendly
read of what just happened. It changes over time as the human
copy improves. CI pipelines and shell scripts shouldn't have to
care: they pin against `--json` and the schema in this document,
which is the part that is frozen.

Concretely, scriptable mode lets you:

- detect upload / download failures by `reason` field rather than
  by `grep`-ing English error text
- pin against a stable exit-code table
- pass the profile passphrase from a file instead of a terminal
  prompt
- pick an alias / container without an interactive picker
- refuse to spawn the agent or auto-unlock — scripts that race
  against an unattended `zz z` are easy to write wrong; we just
  reject the situation up front

---

## Activation (precedence: highest first)

1. **Command-line flag.** `--json` or `--quiet` on the front of
   argv, before the verb.
2. **Environment variable.** `ZZ_OUTPUT=json` switches into JSON
   mode for the whole invocation. `ZZ_OUTPUT=text` is the
   default and effectively a no-op. `ZZ_OUTPUT=quiet` is **not**
   honoured — quiet mode is flag-only, so a script that
   habitually exports `ZZ_OUTPUT=json` doesn't get silently
   pulled into quiet output by an unrelated env override.
3. **Default.** Text output (with ANSI color when stdout is a
   TTY).

There is no "auto-JSON when stdout is non-TTY" heuristic. It
breaks `zz s notes.md | tee log.txt` for an interactive operator
who pipes to `tee` for a transcript, so we keep activation
explicit.

---

## Stdout / stderr policy

In `--json` mode:

- **stdout** carries NDJSON. One JSON object per line, no
  prologue, no trailing comma, no blank lines, no ANSI escapes.
- **stderr** is empty for any command that reaches dispatch. The
  only things that may land there are fatal pre-output errors
  (panic, init failure, global-flag parse error) — these happen
  before the JSON contract is established and degrade to plain
  text.

In `--quiet` mode stdout carries one short line per result;
stderr carries the same kind of pre-output diagnostics.

---

## Mandatory fields on every NDJSON record

| Field | Type | Meaning |
|---|---|---|
| `v` | string | Schema version. Always `"1"` for the contract on this page. |
| `event` | string | Event kind — `uploaded`, `downloaded`, `failed`, `batch_summary`, `unlocked`, `locked`, `wiped`, `doctor_check`, `doctor_summary`. Open the list below for the full set. |
| `ts` | string | UTC timestamp, RFC 3339 with no fractional seconds (`YYYY-MM-DDTHH:MM:SSZ`). |

Field order in the wire form is `v` → `event` → `ts` →
event-specific fields. Pinning a field order is unusual for
JSON; we do it because it lets a fast NDJSON consumer read the
first few bytes of each line to filter without invoking a full
parser. The parser still has to accept any field order — this
is a producer guarantee, not a consumer constraint.

---

## Events

### `uploaded`

One per file that landed on the cloud. Emitted from `zz`,
`zz s`, `zz sa`, `zz sar`, and the bundle variants (`sx`, `sax`,
`sarx`); the bundle path emits exactly one `uploaded` for the
single `.tar.zst` blob, not one per archived file.

```jsonc
{"v":"1","event":"uploaded","ts":"2026-05-16T12:34:56Z",
 "file":"notes.md","bytes":1234,
 "compressed_pct":42,
 "alias":"work-nc","target":"nextcloud.example.org/zz-drop"}
```

- `file` — the final leaf name on the cloud (after any
  rename-on-collision). For the bundle path it is the
  `<dirname>.tar.zst` leaf.
- `bytes` — payload size *as uploaded*. For compressed uploads
  this is the compressed byte count.
- `compressed_pct` — present only when zstd ran for this file.
  Value is "saved %": `0..=99` with `90` meaning a 10× ratio.
  Absent when the upload skipped compression (no `x` modifier,
  or file below the compression threshold).
- `alias` — active profile alias.
- `target` — provider host (or provider kind for cloud-API
  providers) and configured remote root, joined with `/`.

### `downloaded`

One per file pulled from the cloud. Emitted from `zz d`,
`zz dx`, `zz da`, `zz dar`.

```jsonc
{"v":"1","event":"downloaded","ts":"2026-05-16T12:34:56Z",
 "file":"notes.md","bytes":1234,
 "alias":"work-nc","target":"nextcloud.example.org/zz-drop"}
```

The `decompressed → sibling file / extracted bundle` text that
the human path prints is intentionally **not** in the JSON
contract — the consumer reads the bytes on disk to know if the
sibling exists.

### `failed`

Per-file failure inside an unlocked session, or a pre-dispatch
failure when the cause is documented enough to fit the contract.
Has different shapes; see the field rules.

```jsonc
{"v":"1","event":"failed","ts":"2026-05-16T12:34:56Z",
 "file":"big.zip","reason":"provider_error",
 "detail":"413 Payload Too Large",
 "alias":"work-nc","target":"nextcloud.example.org/zz-drop"}
```

- `reason` — closed enum, see the table below.
- `file`, `alias`, `target` — present when the failure occurred
  inside an unlocked session for a specific file. Absent for
  pre-unlock errors (no profile, no passphrase, etc.).
- `detail` — optional free-form string. Producer guarantee: no
  raw control bytes (newlines, ESC, NUL) survive into the wire
  form — serde_json escapes them as `\uXXXX`. Consumer
  guarantee: treat this as informative only; never `eval` or
  shell-interpolate.
- `candidates` — present only when `reason ==
  "alias_ambiguous"`. Array of valid alias strings the operator
  could pass via `--alias`.

### `batch_summary`

The final record of a bulk verb (`zz s` with multiple files,
`zz sa`, `zz da`, etc.). Always last in the stream for that
invocation.

```jsonc
{"v":"1","event":"batch_summary","ts":"2026-05-16T12:34:56Z",
 "total":12,"ok":11,"failed":1,"exit_code":9}
```

- `total` = `ok + failed`. Skipped files (dotfiles, symlinks,
  directories) are NOT counted.
- `exit_code` mirrors the process exit code (so a script can
  read it from the final NDJSON record without checking `$?`).

### `unlocked` / `locked` / `wiped`

State transitions. One record each.

```jsonc
{"v":"1","event":"unlocked","ts":"...","alias":"work-nc","target":"..."}
{"v":"1","event":"locked","ts":"..."}
{"v":"1","event":"wiped","ts":"..."}
```

`zz q` is idempotent: running it against an already-locked agent
still emits `locked` and exits 0. The text mode distinguishes
"locked" from "already locked"; the JSON schema does not.

### `doctor_check` / `doctor_summary`

`zz f` (doctor) emits one `doctor_check` per probe, then a
single `doctor_summary` as the last record.

```jsonc
{"v":"1","event":"doctor_check","ts":"...",
 "name":"agent_unlocked","ok":false,"detail":"no socket"}
{"v":"1","event":"doctor_summary","ts":"...",
 "ok":false,"failed":["agent_unlocked"]}
```

Probes today (additive — more can be added without bumping
the schema version):

- `container_local` — `profiles-local.zz` present?
- `container_remote` — `profiles-remote.zz` present?
- `agent_socket` — Unix socket present in the runtime dir?
- `agent_unlocked` — agent reachable and a profile unlocked?
- `sacs_state` — classifier state (`detail`: `S0`..`S4`).
- `build_id` — current build identity (`detail`: id string).

`doctor_summary.ok` is `true` when no probe contributed to the
`failed` list. Probes that are *informational* (e.g.
`container_local` returning `false` because the operator hasn't
run `zz c` yet) don't enter `failed`.

### `completions_setup` / `completions_status`

`zz --setup-completions` emits a single `completions_setup`
record describing what changed (or didn't). `zz --check-completions`
emits a single `completions_status` record describing the state
on disk.

```jsonc
{"v":"1","event":"completions_setup","ts":"...",
 "shell":"zsh","completion_path":"/home/u/.zfunc/_zz",
 "completion_action":"created",
 "rc_path":"/home/u/.zshrc","rc_action":"inserted",
 "framework":"none",
 "hint":"open a new terminal (or run `exec zsh -l`) ..."}

{"v":"1","event":"completions_status","ts":"...",
 "shell":"zsh","wired":true,"status":"wired",
 "completion_path":"/home/u/.zfunc/_zz",
 "rc_path":"/home/u/.zshrc"}
```

Field values are closed enums:

- `shell`: `"bash" | "zsh" | "fish"`
- `completion_action`: `"created" | "updated" | "unchanged"`
- `rc_action`: `"inserted" | "updated" | "unchanged" | "not_needed"`
- `framework`: `"none" | "oh-my-zsh" | "prezto" | "zinit" | "antibody" | "antidote" | "znap" | "zimfw" | "zplug"`
- `status` (check only): `"wired" | "needs_rc_block" | "missing"`

Exit codes: `0` when `wired` (or setup succeeded), `2` for usage
errors (unknown shell, missing positional with `$SHELL` unset),
`12` (`EXIT_COMPLETIONS_FAILED`) when the filesystem write fails
or `--check-completions` reports anything other than `wired`.

---

## Reason codes (1:1 with exit codes)

The `reason` field on `failed` events is drawn from a closed
enum. Each value maps to exactly one process exit code.

| `reason` | Exit | Meaning |
|---|---:|---|
| `usage` | 2 | Flag/arg parse error or unsupported combination. |
| `not_implemented` | 3 | Verb recognised by the parser but gated off in this build. |
| `agent_unreachable` | 5 | Socket present but not accepting connections, or RPC failed. |
| `profile_missing` | 6 | No `profiles-local.zz` / `profiles-remote.zz` at the expected path. |
| `decrypt_failed` | 7 | Wrong passphrase or corrupted container. |
| `wipe_cancelled` | 8 | Operator declined the wipe prompt (text mode only — scriptable mode rejects without `--yes` instead). |
| `provider_error` | 9 | Upstream provider failed (network, HTTP non-2xx, parse error). |
| `agent_locked` | 10 | Agent reachable but profile locked. Scriptable mode never auto-unlocks. |
| `passphrase_file_permissions` | 11 | Passphrase file mode > 0600 or owner mismatch. |
| `interactive_required` | 2 | Command would prompt (`zz w` without `--yes`). |
| `interactive_only` | 2 | Command is a TUI / wizard (`zz c`, setup) and has no scriptable surface. |
| `alias_ambiguous` | 2 | Multiple aliases in the container and no `--alias` / `ZZ_ALIAS` / cached default. |
| `container_ambiguous` | 2 | Both `local` and `remote` containers exist; no `--local` / `--remote` / `ZZ_CONTAINER` provided. |
| `completions_install_failed` | 12 | `--setup-completions` couldn't write the completion file or the rc-file block (permission denied, disk full, read-only filesystem). |

Exit codes are stable from 1.0.0 onward. New codes can be
*added* (additive, doesn't bump the schema). Removals or
renumberings would be breaking and would bump the schema to
`"2"`.

---

## Global flags

These can appear before the verb on any invocation. After the
first non-flag positional, parsing stops and every remaining
token is verb argument or path. Use `--` to terminate flag
parsing explicitly (`zz -- --weird-name` uploads a file literally
named `--weird-name`).

| Flag | Value | Notes |
|---|---|---|
| `--json` | — | Switch stdout to NDJSON. |
| `--quiet` | — | One terse text line per result. Mutually exclusive with `--json`. |
| `--passphrase-file <path>` | path | Read the profile passphrase from this file instead of prompting. Strict permission check — see the security section. |
| `--alias <name>` | string | Pre-select this alias for unlock and dispatch. Skips the picker. |
| `--local` | — | Force operations against `profiles-local.zz`. |
| `--remote` | — | Force operations against `profiles-remote.zz`. Mutually exclusive with `--local`. |
| `--yes` | — | Auto-confirm destructive prompts. Required for `zz w` in `--json` / `--quiet`. |

The flag form `--name=value` is accepted everywhere
`--name value` is.

---

## Environment variables

Applied when no command-line flag overrides them.

| Var | Type | Default | Override flag |
|---|---|---|---|
| `ZZ_OUTPUT` | `text` \| `json` | `text` | `--json` (higher precedence). `quiet` is flag-only and rejected as an env value. |
| `ZZ_PASSPHRASE_FILE` | path | unset | `--passphrase-file <path>`. |
| `ZZ_ALIAS` | string | unset | `--alias <name>`. |
| `ZZ_CONTAINER` | `local` \| `remote` | unset | `--local` / `--remote`. |
| `ZZ_CONFIG_DIR` | absolute path | OS default | env-only — overrides the *entire* state tree: `<root>/config`, `<root>/cache`, `<root>/runtime`. Relative paths are rejected. |

The `ZZ_PASSPHRASE_FILE` form is the only supported way to feed
a passphrase to a script. There is **no** `ZZ_PASSPHRASE=<value>`
(env values leak via `/proc/<pid>/environ`, `ps eww`,
container debug snapshots) and no `--passphrase-stdin` yet (deferred
— revisit once a real vault-piped flow surfaces).

---

## Passphrase file format

```text
my-passphrase
```

That's it. One line, optionally terminated by exactly one `\n`,
no leading or trailing whitespace stripping beyond that one
newline. Embedded spaces and embedded newlines are preserved
verbatim — they're part of the passphrase.

Validation rules (refusal exits with code `11`):

- Must be a regular file (symlinks, directories, devices, FIFOs
  → refused as `not a regular file`).
- Must be owned by the **current UID**. A file owned by root or
  by another user is rejected even if the current process can
  read it.
- File mode must be `≤ 0600` (group + others bits must be 0).
  `0600` is the common case; `0400` and `0000` are also fine.
- Maximum size: 4096 bytes. A passphrase that hits this cap is
  almost certainly a misuse (e.g. `--passphrase-file` pointed
  at a `.zz` envelope by mistake).
- No NUL byte in the content.

These rules apply on every invocation that consumes the file —
not just on first read.

---

## Auto-unlock policy

In scriptable mode the daily commands **never** auto-unlock the
agent. If the agent is up but the profile is locked,
`zz s file.md --json` emits `failed reason=agent_locked` and
exits `10` instead of prompting for a passphrase. The script
must call `zz z --json --passphrase-file <path>` explicitly
first.

This is the most common gotcha when porting a one-shot
interactive workflow to CI. The exit code 10 is *not* the
generic "agent broken" 5 — it's specifically "the operator has
to unlock first", and it's easy to branch on.

In text mode the existing behaviour is unchanged: if the agent
is down, the operator gets a "run `zz z`" hint.

---

## Alias / container resolution

Scriptable mode resolves the alias deterministically:

1. `--alias <name>` (or `ZZ_ALIAS`)
2. cached default in the sidecar (`last-default-local` /
   `last-default-remote`)
3. single-alias profile (no ambiguity, auto-pick)
4. otherwise: `failed reason=alias_ambiguous` with the candidate
   set on the record

There is no numbered-list picker in scriptable mode — it would
block on stdin.

Same idea for the container selection between `profiles-local.zz`
and `profiles-remote.zz`: explicit flag/env wins, otherwise
auto-pick when only one exists, otherwise
`container_ambiguous`.

---

## CI cookbook

A representative end-to-end script:

```bash
#!/usr/bin/env bash
set -euo pipefail

export ZZ_OUTPUT=json
export ZZ_CONFIG_DIR=/run/secrets/zz-drop
export ZZ_PASSPHRASE_FILE=/run/secrets/zz.pass
export ZZ_ALIAS=ci-bot

# Unlock once at the start of the job. The agent stays up
# inside the build container until idle TTL.
zz z | jq -e '.event=="unlocked"' >/dev/null

# Lock on exit even if a step fails.
trap 'zz q' EXIT

# Upload an artifact, fail the job on provider error.
zz s artifact.zip \
  | tee >(jq -c 'select(.event=="failed")' >&2) \
  | jq -e 'select(.event=="batch_summary").exit_code==0' >/dev/null
```

Notes:

- `jq -e` returns non-zero if the predicate is false, which
  composes with `set -e` to fail the job on a missing event or
  a non-zero `batch_summary.exit_code`.
- The `tee >(... >&2)` pattern echoes failure records to stderr
  for easier debugging without breaking the pipeline.

A second pattern — download with a per-file gate:

```bash
zz d 'reports/*.pdf' --json \
  | jq -r 'select(.event=="downloaded") | .file'   # ← list what landed
```

---

## What's deliberately out of scope

- `--passphrase-stdin` — env values and stdin are both racy in
  CI; the file path keeps the secret on a tmpfs that the script
  controls.
- Progress events for long-running uploads / downloads — a
  later addition. Adding them is additive and won't bump the
  schema.
- Per-file `listed` events on bulk verbs — `zz da` / `zz dar`
  emit one `downloaded` per file; there's no separate listing
  phase visible on the wire.

---

## Stability guarantees

- `v: "1"` is frozen from 1.0.0 onward.
- New fields may appear on existing events (additive). Consumers
  that pin against `v == "1"` must ignore unknown fields.
- New events may be added (additive). Consumers that switch on
  `event` must default-case unknown values without failing.
- The reason enum is closed but extensible: new variants can
  appear in later releases. Consumers must default-case unknown
  reasons.
- Renaming a field, removing an event, or changing a field type
  is breaking. That bumps `v` to `"2"` and the previous schema
  stays available indefinitely behind the version field.

The matching JSON Schema file is at
[`docs/scriptable/zz-drop-output.v1.json`](scriptable/zz-drop-output.v1.json).
