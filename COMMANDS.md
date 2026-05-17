# zz-drop commands (v1)

User-facing manual for the `zz` CLI binary, with worked examples
and output samples. The grammar is deliberately terse —
single-letter verbs that compose with single-letter modifiers —
so daily use stays under your left hand.

**Looking for the verb-table spec?** The canonical grammar
reference (reserved verbs, modifier semantics, parser rule,
exit codes) lives at [`docs/commands.md`](docs/commands.md).
This file builds on top of it with prose and concrete examples.

## Verbs

### Upload — `s` family

| Form | Args | Meaning |
|---|---|---|
| `zz <file>...` | 1+ paths | Upload one or more files. Default verb — no token in front. Files land at `<remote_root>/<basename>`. |
| `zz s <file>...` | 1+ paths | Same as the default. Use the explicit `s` only when the first argument literally clashes with a reserved name (rare). |
| `zz s <file>... <remote-dir>/` | 2+ args, last has trailing `/` | Same as above but the trailing-`/` argument is the **remote sub-directory** under `<remote_root>`. `zz s a.md b.md docs/` → `<remote_root>/docs/{a.md,b.md}`. |
| `zz sx <file>` | 1 path | Compress with zstd (level 3) before upload; the cloud sees `<file>.zst`. Files smaller than 4 KiB skip compression and upload raw. Trailing-`/` rule applies (`zz sx file.md docs/` → `<remote_root>/docs/file.md.zst`). |
| `zz sa <dir>` | 1 directory | Upload every top-level regular file inside `<dir>` as a separate blob. `.` resolves to the current working directory. Hidden files (`.git/`, `.bashrc`) and symlinks are skipped. |
| `zz sa <dir> <remote-prefix>` | 2 args | Like `sa <dir>` but the operator-typed `<remote-prefix>` is prepended to each file's remote path. `zz sa /tmp/proj backup/` → `<remote_root>/backup/{file1,file2,...}`. |
| `zz sar <dir>` | 1 directory | Same as `sa` but recursive — subdirectories preserved on the remote side. |
| `zz sar <dir> <remote-prefix>` | 2 args | Recursive variant with explicit remote sub-prefix. |
| `zz sax <dir>` | 1 directory | Bundle every top-level regular file in `<dir>` into a single archive `<dir-name>.tar.zst` and upload that one blob. The relative paths inside the archive let `dx` recreate the tree on download. |
| `zz sax <dir> <remote-prefix>` | 2 args | Same bundle, placed at `<remote_root>/<remote-prefix>/<dir-name>.tar.zst`. |
| `zz sarx <dir>` | 1 directory | Recursive bundle. |
| `zz sarx <dir> <remote-prefix>` | 2 args | Recursive bundle with explicit remote sub-prefix. |

**Convention recap:**
- For `s`/`sx` (multi-file): the **last argument with a trailing `/`** is the remote destination directory; the rest are sources. Without a trailing-`/` arg, every argument is a source.
- For `sa`/`sar`/`sax`/`sarx`: the **first** positional is the local source dir, the **second** (optional) is the remote sub-prefix. Trailing `/` on either is optional and stripped for normalisation.
- A trailing-`/` argument *alone* on `s` (`zz s docs/`) is rejected with `MissingArgument` — there are no source files.

`<file>` accepts any local path (`./report.pdf`, `/tmp/x.md`, `~/Desktop/notes.txt`). Local paths follow shell rules: relative to cwd unless absolute, `~` is shell-expanded before zz sees it.

### Download — `d` family

| Form | Args | Meaning |
|---|---|---|
| `zz d <name>...` | 1+ remote names | Download each blob's raw bytes. Saved with its basename in cwd. |
| `zz d <name>... <local-dir>/` | 2+ args, last has trailing `/` | Same as above but the trailing-`/` argument is the **local destination directory**. `zz d api.md guide.md ./out/` → `./out/{api.md,guide.md}`. |
| `zz dx <name>` | 1 remote name | Download, then `(a)` if zstd-compressed, decompress alongside (keeps the original `.zst`); `(b)` if the decompressed bytes are a tar archive, extract into a sibling directory named after the bundle. Non-destructive — refuses to extract on top of an existing directory. |
| `zz dx <name> <local-dir>/` | 2+ args, last has trailing `/` | Same but place the downloaded blob (and any decompressed sibling / extracted directory) under `<local-dir>` instead of cwd. |
| `zz da` | — | Download every top-level remote file into cwd. |
| `zz da <local-dest>` | 1 directory | Download every top-level remote file into `<local-dest>`. |
| `zz da <local-dest> <remote-prefix>` | 2 args | Download top-level files **of the remote sub-prefix** into `<local-dest>`. `zz da backup docs/` → only `docs/` top-level files into `cwd/backup`. |
| `zz dar` | — | Recursive variant: mirror the entire remote tree into cwd. |
| `zz dar <local-dest>` | 1 directory | Mirror the entire remote tree into `<local-dest>`. |
| `zz dar <local-dest> <remote-prefix>` | 2 args | Mirror only the remote sub-prefix tree into `<local-dest>`. `zz dar ./snapshot project/build/` → only `project/build/...` mirrored under `./snapshot`. |
| `zz dax` / `zz darx` | — | Bulk download with per-file decompression. Not implemented yet — today returns `EXIT_NOT_IMPLEMENTED` with a hint to use `dx <bundle>.tar.zst` for the symmetric form. |

**Convention recap:**
- For `d`/`dx` (multi-file): same trailing-`/` rule as `s` — the last argument with a trailing `/` is the **local destination directory**.
- For `da`/`dar`/`dax`/`darx`: the **first** positional is the local destination (defaults to cwd), the **second** (optional) is the remote sub-prefix. Both args are optional. Trailing `/` on either is optional and stripped.
- The `<remote-prefix>` is **always** relative to the profile's `<remote_root>`. No `..`, no leading `/`.
- Local paths (`<local-dest>`, `<file>`) follow shell rules. Remote names / prefixes are slash-separated relative paths.

Download `.tar.zst` example:

```sh
$ zz dx mydir.tar.zst
downloaded mydir.tar.zst 12 KiB ← casa-nc · cloud.example.org/zz-drop
  · extracted bundle into ./mydir/
```

The `.tar.zst` blob stays on disk; the extracted tree sits next to it under `mydir/`.

### Container — `z`, `q`, `w`

| Form | Args | Meaning |
|---|---|---|
| `zz z` | — | Open `profiles-local.zz`, prompt for the passphrase and unlock the agent. If the container holds more than one inner profile, a numbered picker appears. |
| `zz q` | — | Lock the agent: zeroize the KEK and the in-RAM container, drop active alias. |
| `zz w` | — | Wipe local zz-drop state. Asks for typed `y` confirmation, then removes `profiles-local.zz`, the `last-default-local` sidecar, the agent socket + token, and the runtime directory. |

### Other

| Form | Args | Meaning |
|---|---|---|
| `zz c` | — | Launch the `zz-tui` configuration interface. Resolves `zz-tui` on `$PATH` — install it next to `zz` for things to work. |
| `zz f` | — | Doctor / diagnostics. **Stub today** — returns `EXIT_NOT_IMPLEMENTED` until the implementation lands. |

## Modifiers (`s` and `d` only)

The composite verbs `s` and `d` accept a set of single-letter modifiers in any order. The pipeline is fixed; only the *set* of letters matters.

| Letter | Meaning | Constraints |
|---|---|---|
| `a` | Bulk: operate on the entire `<dir>` rather than a specific file. Requires the `<dir>` argument. |
| `r` | Recurse. Only meaningful when paired with `a` — `sr`/`dr` alone are rejected. |
| `x` | Compress on upload (zstd, level 3). Decompress on download. Bundles for `sa+x` / `sar+x`. |
| `e` | Reserved for a future encryption modifier; not implemented in v1. Today rejected explicitly with the message *"encryption (`e`) is not implemented in v1"* so the operator sees something useful instead of a generic parse error. |

Set semantics — equivalent forms produce the same command:

```text
zz sar .   ≡   zz sra .   ≡   zz ras .         (no compression)
zz sarx .  ≡   zz sxar .  ≡   zz sxra .  ≡  …  (with compression)
zz darx .  ≡   zz drax .  ≡   zz dxra .  ≡  …
```

Pipeline order on upload (deterministic, regardless of letter order):

```text
file(s)  →  [tar bundle if a/r ∧ x]
         →  [zstd if x]
         →  upload as <name>(.zst|.tar.zst)
```

If the `e` modifier ever lands, compression will run before encryption — encrypted bytes are high-entropy and won't compress further.

## Errors

### Parser

| Input | Error |
|---|---|
| `zz` | `no arguments; run zz <file> to upload, or zz f for diagnostics` |
| `zz s` | `s requires at least one argument` |
| `zz sa` | `sa requires at least one argument` |
| `zz sa . backup/ extra` | `sa takes no arguments` (3rd positional rejected — `sa` accepts at most local-dir + remote-prefix) |
| `zz s docs/` | `s requires at least one argument` (a trailing-`/` arg alone is a destination, no source) |
| `zz saa .` | `saa: modifier a repeated (set semantics — at most once)` |
| `zz sex file.md` | `sex: encryption (e) is not implemented in v1; v1 supports a, r, x` |
| `zz sr` / `zz dr x` | `sr: unknown modifier r` (`r` requires `a`) |

Anything that doesn't match a reserved verb is treated as an upload path, so `zz x` uploads a file literally named `x`. Use `./x` if your filename collides with what looks like a verb.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `2` | Usage error (missing args, malformed command) |
| `3` | Recognized but not implemented yet (e.g. `zz dax .`) |
| `5` | Agent unreachable (socket missing, refused, handshake failed) |
| `6` | Profile not found |
| `7` | Decryption failed (wrong passphrase or corrupt container) |
| `8` | Wipe cancelled by the operator |
| `9` | Provider error (Nextcloud / Google Drive / OneDrive returning an error response) |
| `127` | `zz c` could not find `zz-tui` on `$PATH` |

## Examples

```sh
# Daily upload of a single file
zz README.md

# Upload several files at once
zz a.md b.md c.md

# Upload several files into a remote sub-directory
zz s a.md b.md c.md docs/

# Compress a large log on the way up
zz sx benchmark.log

# Compress and place under a remote sub-directory
zz sx benchmark.log logs/

# Upload all top-level files in cwd as separate blobs
zz sa .

# Same, but place them under <remote_root>/backup/
zz sa . backup/

# Upload the whole project as ONE compressed bundle (under root)
zz sarx .

# Same bundle, placed at <remote_root>/snapshots/<dirname>.tar.zst
zz sarx . snapshots/

# Download a single file (lands as ./notes.md)
zz d notes.md

# Download multiple files into a specific local dir
zz d api.md guide.md ./out/

# Download a compressed file and decompress it
zz dx benchmark.log.zst

# Download a bundle and extract it into a sibling directory
zz dx myproject.tar.zst        # produces myproject/

# Download the entire remote tree into cwd (zero-arg form)
zz dar

# Mirror only the remote docs/ subtree into ./local-docs
zz dar ./local-docs docs/

# Unlock the local container, lock when done
zz z
zz q

# Walk away — the agent auto-locks after 10 minutes
```

## Shell completion

`zz` ships its own state-aware completion engine ("SACS"). The shell
script is dumb (~30-40 lines), inlined in the binary; the actual
ranking lives in `zz` and updates the moment you upgrade. Three
shells supported: bash, zsh, fish.

```sh
# bash — load the script in your current shell, then arrange to do
# it on every login (e.g. via ~/.bashrc):
zz --completions bash | source

# zsh — drop the function file into a directory on your fpath:
mkdir -p ~/.zfunc
zz --completions zsh > ~/.zfunc/_zz
fpath+=( ~/.zfunc )
autoload -U compinit && compinit
# nicer rendering (menu select, headed sections, colors) is opt-in:
# see `docs/sacs.md` → "zsh styling (opt-in)".

# fish — install once into the standard fish completions dir:
zz --completions fish > ~/.config/fish/completions/zz.fish
```

Once installed, pressing TAB after `zz` in your shell will show
contextual candidates: file names while you compose an upload,
remote names when you download, sub-directory prefixes when you
bulk-transfer, and so on. Behaviour adapts to the current state
of zz-drop:

| Container | Agent | What TAB suggests |
|---|---|---|
| missing | — | the setup verbs (`c`, `--help`) |
| present | down | the unlock verbs (`z`, `c`, `q`, `w`, `f`) |
| present | locked | same as above |
| present | unlocked | local files first, then verbs |

See [`docs/sacs.md`](docs/sacs.md) for the architecture, the
full ranking table, and the latency caveats. The `--help` page
(a static fallback that does not need the completion installed)
is also produced by the binary:

```sh
zz --help          # static cheat sheet (or zz -h)
```

## Scripting

`zz` has a stable scriptable contract for CI and shell scripts:

```sh
# JSON mode — one NDJSON record per result on stdout.
zz s artifact.zip --json
zz d 'reports/*.pdf' --json | jq -r 'select(.event=="downloaded") | .file'

# Quiet mode — one terse text line per result, no ANSI.
zz s notes.md --quiet
```

The minimum viable CI pattern (passphrase from a file, no
prompts, no auto-unlock):

```sh
export ZZ_OUTPUT=json
export ZZ_PASSPHRASE_FILE=/run/secrets/zz.pass
export ZZ_ALIAS=ci-bot

zz z                       # unlock once
trap 'zz q' EXIT           # always lock on exit
zz s artifact.zip          # upload
```

Three things to know up front:

1. **No auto-unlock in `--json` / `--quiet`.** If the agent is
   locked when a daily command runs, the failure is
   `reason=agent_locked` with exit code `10` — fix by calling
   `zz z` first.
2. **Destructive verbs need `--yes`.** `zz w` rejects with
   `interactive_required` unless `--yes` (or
   `ZZ_DROP_CONFIRM_WIPE=yes`) is set.
3. **The TUI is not scriptable.** `zz c` exits `2` with
   `interactive_only` under `--json` / `--quiet`.

Full contract — flags, env vars, NDJSON event schema, exit
code table, CI cookbook, stability guarantees:
[`docs/scriptable.md`](docs/scriptable.md).

Worked examples by verb (single file, bulk, recursive, glob,
bundle, decompress, doctor):
[`docs/usage.md`](docs/usage.md).

## Cross-references

- [`README.md`](README.md) — install, status, security headlines.
- [`SECURITY.md`](SECURITY.md) — disclosure policy and per-binary security posture.
- [`docs/security.md`](docs/security.md) — threat model, what the server sees / does not see.
- [`docs/agent.md`](docs/agent.md) — local agent: lifecycle, socket auth.
- [`docs/sacs.md`](docs/sacs.md) — shell completion: states, ranking, NDJSON schema.
- [`docs/commands.md`](docs/commands.md) — canonical grammar spec: reserved verbs, modifier semantics, parser rule.
- [`docs/scriptable.md`](docs/scriptable.md) — full `--json` / `--quiet` contract.
- [`docs/usage.md`](docs/usage.md) — worked examples for every verb.
