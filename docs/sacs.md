# SACS — state-aware contextual suggestions

`zz` ships its own shell completion engine. The shell script is dumb
(`zz --completions <shell>` produces ~30-40 lines per shell) and
relays to a hidden subcommand, `zz __complete <args>`, that owns all
the ranking and filesystem / agent observation. The brain is the
binary: rebuilding `zz` is the only thing needed to update the
suggestions, the script never changes.

## Installation

```sh
zz --completions bash | source                                  # bash
zz --completions zsh  > ~/.zfunc/_zz                            # zsh (then `autoload -U compinit && compinit`)
zz --completions fish > ~/.config/fish/completions/zz.fish      # fish
```

Each script registers completion for both `zz` and `zz-drop` so it
works the same regardless of which name the operator invokes.

## zsh styling (opt-in)

Out of the box, the zsh completer renders a flat list — same as
the other two shells. zsh's `zstyle` machinery is **context-aware**:
patterns like `:completion:*:*:(zz|zz-drop):*` match only when the
user is completing `zz` or `zz-drop`, never any other command. So
a richer presentation is achievable scoped strictly to `zz`,
without touching the TAB experience of `git`, `ls`, `cd`,
`kubectl`, or anything else.

The completer already groups candidates by kind (`verb`,
`atomic`, `file_remote`, `dir_remote`, `file_local`, `dir_local`,
`help`) via `_describe -t <tag>`. Adding the six lines below to
`~/.zshrc` turns those tags into headed sections with menu select
and filename colors:

```zsh
# zz-drop completion — every line below is either neutral
# (file paths, module load, widget registration) or scoped
# explicitly to the (zz|zz-drop) command context. No global
# zsh setting is altered, so other commands' TAB experience
# (git, ls, kubectl, …) is left exactly as you had it.
fpath=(~/.zfunc $fpath)
zmodload zsh/complist
autoload -U compinit && compinit -i
# On stock macOS zsh, loading zsh/complist does not auto-register
# the menu-select widget; the explicit `zle -C` ensures the
# widget exists so the scoped zstyle below can activate it.
zle -C menu-select .menu-select _main_complete

# All zstyles are scoped to the (zz|zz-drop) command context only.
zstyle ':completion:*:*:(zz|zz-drop):*' menu select=1
zstyle ':completion:*:*:(zz|zz-drop):*' group-name ''
zstyle ':completion:*:*:(zz|zz-drop):*:descriptions' format '%F{cyan}[%d]%f'
zstyle ':completion:*:*:(zz|zz-drop):*' verbose yes
zstyle ':completion:*:*:(zz|zz-drop):*' sort false
zstyle ':completion:*:*:(zz|zz-drop):*' list-colors "${(s.:.)LS_COLORS}" 'ma=01;36'
zstyle ':completion:*:*:(zz|zz-drop):*' tag-order \
    'remote-files remote-dirs local-files local-dirs verbs atomics help'
```

Line by line:

| Line | Effect | Scope |
|---|---|---|
| `fpath=(~/.zfunc $fpath)` + `compinit -i` | Make zsh discover the SACS completion file installed under `~/.zfunc/_zz`. | Neutral — `~/.zfunc` only contains zz files. |
| `zmodload zsh/complist` + `zle -C menu-select` | Load complist and register the `menu-select` widget. On stock macOS zsh the widget is not auto-registered when complist loads, so the explicit `zle -C` is needed for menu-select to activate at all. | Neutral — registers a widget that is invoked only when a zstyle requests menu-select. No key is rebound globally. |
| `menu select=1` | Tell zsh's completer to enter menu-select on the very first ambiguous match (≥1 candidate). | **Scoped to `(zz|zz-drop)` only**. Other commands keep their default zsh completion behaviour. |
| `group-name ''` | Render each `_describe -t <tag>` group as a separate section. | Scoped. |
| `descriptions … format` | Cyan header above each section (`[remote files]`, `[verbs]`, …). | Scoped. |
| `verbose yes` | Show the description string next to each value. | Scoped. |
| `sort false` | Preserve SACS's rank order (newest-first for files, fixed verb order); without this zsh re-sorts each group alphabetically. | Scoped. |
| `list-colors … 'ma=01;36'` | Apply your `$LS_COLORS` (filename colors by extension) and highlight the currently-selected entry in bold cyan. | Scoped. |
| `tag-order` | Section order: remote results first, then local, then verbs, then atomics. | Scoped. |

### Per-shell wrapper for `zz d <pattern>` (download glob)

`zz d` supports remote glob patterns server-side (see
[`commands.md`](./commands.md#path-semantics)), but every shell
runs its glob engine against the *local* filesystem first.
That creates an asymmetry between upload and download verbs:

- `zz s A*` works because the shell finds local files starting
  with `A`, expands them, and passes the concrete names to
  `zz s`. The binary never sees the `*`.
- `zz d Q*` fails (or downloads the wrong file) because the
  shell tries to match `Q*` against the local cwd, which is
  the wrong filesystem entirely.

To restore symmetry — `zz d <pattern>` should reach the binary
literally, then `zz d` expands it server-side against the
remote listing — use the per-shell wrapper below. Each wrapper
is **scoped to `zz`**: only the `zz` command word is
intercepted, every other command in the shell is untouched.

#### zsh

```zsh
# In ~/.zshrc (after the completion block).
# Skip zsh's local-fs glob for zz download verbs (d, dx, da, darx, …).
# Upload verbs (s, sa, sx, …) keep zsh's normal local expansion.
zz() {
    case "$1" in
        d|d[a-z]*) noglob command zz "$@" ;;
        *)         command zz "$@" ;;
    esac
}
```

#### bash

bash's default behaviour is more forgiving than zsh's: when a
glob has no local matches, bash passes the literal pattern to
the command. So `zz d Q*` typed in a cwd with no `Q*` files
already reaches `zz d` correctly under default bash. The wrapper
below handles the two remaining cases — local files happen to
match the pattern, or `shopt -s failglob` is set — by always
disabling the local glob for download verbs:

```bash
# In ~/.bashrc, after `eval "$(zz --completions bash)"`.
zz() {
    case "$1" in
        d|d[a-z]*)
            set -f
            command zz "$@"
            local rc=$?
            set +f
            return "$rc"
            ;;
        *)
            command zz "$@"
            ;;
    esac
}
```

Note: bash expands the wrapper's positional arguments *before*
calling the function body, so `set -f` only suppresses
expansion for *subsequent* commands in the function. To benefit
from it, callers should still quote ambiguous patterns
(`zz d 'Q*'`). If you prefer absolute safety over keystroke
ergonomics, the universally portable workaround is just
quoting: `zz d 'Q*'` works in every shell without any wrapper.

#### fish

fish errors on unmatched globs by default (similar to zsh). Add
a function to `~/.config/fish/functions/zz.fish`:

```fish
function zz --description 'zz wrapper: skip local glob on download verbs'
    switch $argv[1]
        case 'd' 'd?*'
            command zz (string escape -- $argv)
        case '*'
            command zz $argv
    end
end
```

`string escape` quotes each argument so fish's glob doesn't
expand it; the binary then receives the literal pattern and
expands remotely.

#### Universal fallback (no wrapper)

If you'd rather not install a wrapper at all, the workaround
that works in every shell is to quote the pattern at the call
site:

```sh
zz d 'Q*'
zz d 'backup/*.pdf'
```

Single quotes prevent every shell's glob engine from touching
the argument; the binary handles the expansion server-side.

**Before** (zsh without the zstyle block):

```
$ zz d <TAB>
AGENTS.md                                 -- download (raw bytes) · remote · 2.7 KiB
AN14880-android-porting.pdf               -- download (raw bytes) · remote · 11.0 MiB
CLAUDE.md                                 -- download (raw bytes) · remote · 1.4 KiB
…
```

**After** (zstyle block in `~/.zshrc`, scoped to `(zz|zz-drop)`):

```
$ zz d <TAB>
[remote file]
▌ AGENTS.md                                  download (raw bytes) · remote · 2.7 KiB
  AN14880-android-porting.pdf                download (raw bytes) · remote · 11.0 MiB
  CLAUDE.md                                  download (raw bytes) · remote · 1.4 KiB
  README.md                                  download (raw bytes) · remote · 4.2 KiB

[verb]
  dx                                         + decompress
  darx                                       + recursive + decompress
```

### What this does NOT do, by design

The block above is the **only** machinery `zz` recommends for
visual polish in zsh. Specifically, it is **not** any of:

- a plugin manager (oh-my-zsh, zinit, antigen, sheldon, zplug,
  znap)
- a third-party plugin (fzf-tab, zsh-syntax-highlighting,
  zsh-autosuggestions)
- an extra binary on `$PATH` (fzf, jq, gum, skim)
- a TAB rebind (`bindkey '^I' …`) or any other key binding
- a custom zle widget

Each of those would either intercept TAB globally (changing the
behaviour of *every* command, not just `zz`) or pull in
third-party software outside the user's control. The
`zstyle :zz|zz-drop:` scope above stays inside zsh's own
completion system and applies to nothing else.

## Architecture

```
shell (bash/zsh/fish)
    |
    | TAB
    v
inlined script (~30-40 lines)
    |
    | invokes:  zz __complete <prev_args> <current_word>
    v
zz binary
    |
    +-- state detector (S0-S4)         filesystem + agent socket probe
    +-- completion provider            exhaustive match on Command enum
    +-- agent bridge                   LIST_REMOTE through the local agent
    |
    | stdout: NDJSON (one candidate per line)
    v
script formats for the shell
    v
shell renders the dropdown in its native style
```

Zero external dependencies: NDJSON is hand-emitted (escaped quotes,
backslashes, control chars), there is no JSON parser involved on
either side, no `clap_complete`, no extra runtime.

`zz __complete` is hidden by the `__` prefix and intercepted in
`main.rs` *before* `parse_args` runs. Tokens like `--help`,
`-h`, `--completions`, `__complete` are matched as exact first
arguments only — anything else falls through to the grammar
parser, so files literally named `--foo` or `__bar` continue to
upload exactly like a file named `q` would (`zz ./q`).

## States and ranking

The detector reads four signals on every TAB:

- `profiles-local.zz` exists on disk?
- `profiles-remote.zz` exists on disk? (only meaningful in builds
  with the `remote` Cargo feature; default v1 builds treat the file
  as dead weight)
- agent socket present?
- agent unlocked? (resolved with one cheap `Status` round-trip when
  the socket is present; skipped otherwise)

| State | Signals | Primary candidates (top → bottom) |
|---|---|---|
| **S0 fresh** | no usable container | `c` · `--help` |
| **S1 down** | container present, no socket | `z` · `c` · `q` · `w` · `f` |
| **S2 locked** | socket present, agent locked | `z` · `q` · `c` · `w` · `f` |
| **S3 ready** | socket present, agent unlocked | local files (newest first) · composite verbs · atomic verbs · `z` |
| **S4 ready, dual** | both containers usable | same as S3 plus `z local` / `z remote` |

The ranking is **fixed and deterministic**. No heuristics, no
machine learning. Same input always produces the same NDJSON; tests
snapshot ~20 contexts in `tests::sacs_complete::*` to keep
regressions visible.

In states S0–S2 the cursor context is ignored intentionally: the
operator cannot usefully upload until they unlock, so the dropdown
stays focused on the unlock/setup verbs even when the user starts
typing `s` or `d`.

In S3/S4 the second-positional rules kick in:

| Cursor context | Candidates |
|---|---|
| `zz <TAB>` | local files newest-first (rank 1..), then local directories with trailing `/` (rank 50..), then composite verbs (rank 100..). Path navigation works — `zz A<TAB>` → `Applications/`, then `zz Applications/<TAB>` lists its contents. |
| `zz s<TAB>` / `zz d<TAB>` | only verb expansions (M2 / M3) |
| `zz s <TAB>` | local files (rank 1..) **+ local directories** (rank 100..). Dirs end with `/`; a follow-up TAB descends into them — see "Path navigation" below |
| `zz s file1.md <TAB>` | more local files (rank 1..) and `dir_remote` "close as destination" (rank 50..) |
| `zz sa <TAB>` | local directories only |
| `zz sa src <TAB>` | remote-prefix candidates from the agent |
| `zz d <TAB>` | remote files (rank 1..) + remote directories (rank 50..) so the operator can navigate. Dir values carry a trailing `/`. Locked agent → empty list, no error. |
| `zz d notes.md <TAB>` | more remote files plus local-dir "close as destination" |
| `zz da <TAB>` | local directories (download destination) |
| `zz da ./out <TAB>` | remote-prefix candidates |

### Path navigation in local-file contexts

Whenever the dropdown lists local files or directories, the typed
prefix can include a path. SACS splits on the last `/`: it reads
the parent directory specified by the path part and matches the
trailing basename against entries there. Example:

- `zz s docs/<TAB>` → contents of `<cwd>/docs/`, with returned
  values like `docs/api.md` and `docs/internal/`.
- `zz s docs/ap<TAB>` → only entries in `<cwd>/docs/` whose
  basename starts with `ap`.

Directory candidates always carry a trailing `/`, so each TAB
either commits to a file or descends one level deeper. There is
no special path syntax to learn — it behaves like the standard
shell completion for `cat <path>`.

## NDJSON schema

One JSON object per line on stdout. Field order is fixed; the
output is sorted by `rank` ascending, then `value` ascending, so
snapshot tests cannot flap.

```
{"value": "...", "display": "...", "description": "...", "kind": "...", "rank": <u32>}
```

Fields:

- **`value`** — the literal text inserted into the buffer.
- **`display`** — the label rendered in the dropdown row, including
  the `zz <verb>` prefix to help the operator confirm what they are
  building.
- **`description`** — a short hint shown to the right of the
  display, when the shell supports it. May be empty.
- **`kind`** — one of:
  - `verb` — composite upload/download verb (`s`, `sx`, `sarx`, …)
  - `atomic` — single-word atomic verb (`z`, `q`, `w`, `c`, `f`)
  - `file_local` — a file in the operator's cwd
  - `dir_local` — a directory in the operator's cwd
  - `file_remote` — a remote file from the agent's `LIST_REMOTE`
  - `dir_remote` — a remote directory
  - `help` — link to the static help (`--help`)
  - `footer` — `+N more matches — keep typing to narrow ...`
- **`rank`** — sorting key. Lower numbers sort first.

## Agent endpoint `LIST_REMOTE`

Remote candidates come from a new agent endpoint added in protocol
version 1 (additive variant — the older request/response set still
round-trips unchanged). The wire-level details are documented in
[`core/docs/agent-protocol.md`](../../core/docs/agent-protocol.md).

Operationally:

- **Cache TTL: 60 seconds.** A miss triggers one provider list
  call; a hit returns from RAM. The agent never persists this
  cache; it lives only between unlock and lock/TTL/exit.
- **Walk-to-root invalidation.** A successful upload to
  `<remote_root>/backup/snap/file.md` drops every cached entry for
  prefixes `backup/snap`, `backup`, and `None` (root), regardless
  of `kind_filter`. The CLI sends one `InvalidateRemote { prefix }`
  request after upload; the agent walks the chain on its side.
- **Locked → empty list.** With a locked agent the endpoint
  returns `Error(NotUnlocked)`; the completion treats it as "no
  remote candidates this time" and never surfaces an error.
- **Provider errors are NOT cached.** A transient 503 from
  Nextcloud / Drive / Graph is returned as `Error(Io { ... })` and
  dropped on the floor by the completer; the next TAB tries again.
- **Hard cap = 200 entries per response.** The agent enforces this
  at write time so the dropdown never receives more than the
  shell can render. The CLI in turn renders at most 50 candidates
  per dropdown position (`REMOTE_CANDIDATE_LIMIT` /
  `LOCAL_CANDIDATE_LIMIT`); spill is summarised by a `footer`
  row so the operator knows there is more material if they keep
  typing to narrow.

## Latency

Target: **under 50 ms perceived per TAB**, but with one big caveat.

- **Cache hit:** state probe + cache lookup + NDJSON render.
  Comfortably under 50 ms on Linux/macOS.
- **First miss against a real provider:** bounded by the provider's
  list call. Nextcloud PROPFIND is typically 50-200 ms; Google
  Drive `files.list` 100-500 ms; OneDrive Graph `/children`
  comparable. The agent serves the second TAB on the same prefix
  from cache for the next 60 s.
- **Agent locked / socket missing:** state probe is filesystem-only
  and stays under 5 ms. No network is touched.
- **Single-threaded accept loop:** the agent processes one request
  at a time (consistent with v1 posture). A long PROPFIND from one
  TAB will queue concurrent `Status` / `GetProfile` calls behind
  it. Acceptable for v1; the `docs/agent.md` lifecycle section
  documents the model.

## What SACS is not

- Not a REPL inside `zz`. There is no prompt, no readline, no
  history. The shell guides input; SACS only feeds the dropdown.
- Not AI / not heuristic. Same arguments → same NDJSON.
- Does not change `zz <file>` default output. A successful upload
  still prints `uploaded ...` with no trailing tip — the
  completion is purely a discovery surface.
- Does not bypass the parser. The grammar in `cli/parser.rs` is
  still the single source of truth; the completion provider
  exhaustive-matches `Command` so any future grammar change must
  update the suggestions.
- Does not include filename matching at invocation time
  (`zz d read` → `readme.md`). That idea operates on the resolved
  command, not on the dropdown.

## Known limitations (v1)

- **No inflight dedup.** A user typing fast may trigger several
  concurrent `LIST_REMOTE` requests for the same prefix on a cold
  cache. The agent serves them sequentially; the second one
  completes against the warm cache. Acceptable in practice.
- **No mtime in remote candidates yet.** The wire schema carries
  `mtime_secs: Option<u64>` but the agent currently always emits
  `None`. Provider integrations can populate this without a
  protocol bump.
- **bash basic** (no `bash-completion` v2) decays to a flat list
  without per-row descriptions. zsh groups by kind via
  `_describe -t <tag>` (and styles further with the optional
  block in `## zsh styling`); fish renders the full layout
  natively.
- **PowerShell, nushell, elvish are not supported.**
- **No "drill into a verb" UX.** Once the operator is inside the
  menu-select dropdown of `zz <TAB>` and arrows down to e.g. the
  `s` row, pressing TAB *commits* `s` (zsh's standard menu-select
  behaviour) instead of expanding it into its modifier variants
  (`sa`, `sx`, `sarx`). Today all variants are already listed
  flat in `zz <TAB>` next to `s`, so the operator can arrow once
  more to reach the desired form. A genuine "expand the
  highlighted verb" gesture would need a custom zle widget that
  re-invokes SACS with the highlighted token as a synthetic
  prefix, which is out of scope for v1.

## Privacy

Snapshots used in tests use only a fixed sanitised identifier set:
`casa-nc`, `cloud.example.org`, `zz-drop`, `alice@example.org`,
and a small filename pool (`readme.md`, `notes.txt`, `draft.md`,
`changelog.md`, `benchmark.log`, `file1.md`, `file2.md`,
`snapshot.tar.zst`). The
`tests::no_secret_or_remote_host_*` lock tests refuse the official
hosted-service hostname, real-looking emails, or token-shaped
strings in any emitted candidate. Nothing in the binary refers to
the official `zz-drop.net` host outside of the `remote` Cargo
feature.

The remote names returned by `LIST_REMOTE` flow through the
dropdown the same way they flow through `zz d <name>` today — they
are not encrypted in v1 (consistent with the rest of the
filename surface). If filename E2EE arrives in a future milestone,
SACS will inherit it without protocol changes.
