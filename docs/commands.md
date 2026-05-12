# zz-drop CLI commands

Canonical command table for the `zz` binary. The user-facing
manual with examples lives in
[`zz-drop/COMMANDS.md`](../COMMANDS.md); this page is
the spec.

## Reserved verbs

| Verb | Args | Meaning |
|---|---|---|
| (none) | 1+ paths | Upload (default — first non-reserved token wins) |
| `s` | 1+ paths | Upload (explicit alias of the default) |
| `d` | 1+ remote names | Download |
| `z` | none / `local` / `remote` | Unlock the active container into the agent |
| `q` | — | Lock (zeroize the in-RAM container) |
| `w` | — | Wipe local zz-drop state (with typed `y` confirmation) |
| `c` | — | Launch the configuration TUI (`zz-tui` on `$PATH`) |
| `f` | — | Doctor / diagnostics |

## Bulk variants — `s` / `d` with the `a` modifier

`a` switches to bulk mode: a single positional `<dir>` argument
is required (the operator types `.` for cwd).

| Verb | Args | Meaning |
|---|---|---|
| `sa` | `<dir>` | Upload top-level regular files of `<dir>` (non-recursive) |
| `sar` | `<dir>` | Upload all files under `<dir>` (recursive, preserves relative path) |
| `da` | `<dest>` | Download all top-level remote files into `<dest>` |
| `dar` | `<dest>` | Download the remote tree into `<dest>` (recursive) |

## Modifiers

The composite verbs `s` and `d` accept a *set* of single-letter
modifiers in any order. Set semantics: `sar` ≡ `sra`, `sarx` ≡
`sxar` ≡ `sxra`, etc.

| Letter | On `s` | On `d` | v1 status |
|---|---|---|---|
| `a` | bulk: needs `<dir>` | bulk: needs `<dest>` | implemented |
| `r` | recurse (with `a` only) | recurse (with `a` only) | implemented |
| `x` | zstd-compress; bundle as `.tar.zst` when paired with `a` | decompress (and extract `.tar.zst` into a sibling dir) | implemented for single-file (`sx` / `dx`) and bundle upload (`sax` / `sarx`); bulk decompress (`dax` / `darx`) returns `EXIT_NOT_IMPLEMENTED` |
| `e` | encrypt with file-content E2EE | (no `e` on `d` — download is always magic-detected) | rejected with explicit "coming in v1.1" message |

`r` without `a` is rejected (`zz sr` / `zz drx` → `unknown
modifier r`). Duplicate letters are rejected (`zz saa` →
`modifier a repeated`).

## Pipeline (deterministic, regardless of letter order)

**Upload (`s` family):**

```
file(s)  →  [tar bundle if (a or ar) and x]
         →  [zstd if x]
         →  upload as <name>(.zst|.tar.zst)
```

**Download (`d` family):**

```
remote bytes
  → [zstd-decompress if x and bytes start with zstd magic]
  → [extract into sibling dir if decompressed bytes have tar ustar magic at offset 257]
  → write
```

Compression always precedes encryption (when `e` graduates in
v1.1) — encrypted bytes are high-entropy and won't compress
further.

## Parser rule

If the first argument exactly matches a reserved verb (or one
of the composite forms `s[a r x]+` / `d[a r x]+`), it is the
command. Otherwise the entire argv is treated as upload paths.

To upload a file whose name happens to match a verb:

```bash
zz ./z
zz ./w
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `2` | Usage error |
| `3` | Recognized but not implemented yet |
| `5` | Agent unreachable |
| `6` | Profile not found |
| `7` | Decryption failed |
| `8` | Wipe cancelled |
| `9` | Provider error (Nextcloud / Google Drive / etc.) |
| `127` | `zz c` could not find `zz-tui` on `$PATH` |
