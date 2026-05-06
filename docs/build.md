# Build & install zz-drop from source

From a clean macOS or Linux machine to a working `zz` with shell
completion. No prior Rust knowledge needed beyond `cargo`.

## TL;DR

```sh
export ZZ_HOME="$HOME/zz-project"           # any directory; the three repos must end up siblings inside it
mkdir -p "$ZZ_HOME" && cd "$ZZ_HOME"

git clone https://github.com/Gibbio/zz-drop-core
git clone https://github.com/Gibbio/zz-drop
git clone https://github.com/Gibbio/zz-drop-tui     # optional, only for `zz c`

( cd zz-drop     && cargo build --release )
( cd zz-drop-tui && cargo build --release ) 2>/dev/null

mkdir -p "$HOME/.local/bin"
ln -sf "$ZZ_HOME/zz-drop/target/release/zz-drop"          "$HOME/.local/bin/zz-drop"
ln -sf "$ZZ_HOME/zz-drop-tui/target/release/zz-drop-tui"  "$HOME/.local/bin/zz-drop-tui" 2>/dev/null
command -v zz >/dev/null 2>&1 \
    && echo "note: 'zz' already on PATH at $(command -v zz); short alias not installed" \
    || ln -sf "$HOME/.local/bin/zz-drop" "$HOME/.local/bin/zz"

mkdir -p ~/.zfunc && zz --completions zsh > ~/.zfunc/_zz   # see docs/sacs.md for the rc block
```

Verify: `zz --help`, `zz f`.

## Prerequisites

- Rust ≥ 1.85 (edition 2024) — install via [rustup.rs](https://rustup.rs/).
- git.
- Linux only: a C toolchain + OpenSSL headers.
  - Debian/Ubuntu: `sudo apt install build-essential pkg-config libssl-dev`
  - Fedora: `sudo dnf install gcc pkgconf-pkg-config openssl-devel`
  - Arch: `sudo pacman -S base-devel openssl`

## Repository layout

`zz-drop` and `zz-drop-tui` reference `zz-drop-core` via
`path = "../zz-drop-core"`, so **all three repos must live as
siblings under the same parent directory**. The parent itself
(`$ZZ_HOME` in this guide) can be anywhere.

```
$ZZ_HOME/
├── zz-drop-core/   ← shared types, crypto, agent protocol     REQUIRED
├── zz-drop/        ← CLI + agent (the binary you'll run)      REQUIRED
└── zz-drop-tui/    ← TUI binary launched by `zz c`            OPTIONAL
```

The three repos are independent (no submodules). Clone in
parallel:

```sh
export ZZ_HOME="$HOME/zz-project"   # change to taste
mkdir -p "$ZZ_HOME" && cd "$ZZ_HOME"

git clone https://github.com/Gibbio/zz-drop-core
git clone https://github.com/Gibbio/zz-drop
git clone https://github.com/Gibbio/zz-drop-tui   # skip for CLI-only setup
```

## Build

```sh
( cd "$ZZ_HOME/zz-drop"     && cargo build --release )    # → target/release/zz-drop
( cd "$ZZ_HOME/zz-drop-tui" && cargo build --release )    # → target/release/zz-drop-tui (optional)
```

Cold build of `zz-drop` is ~3–5 min; warm rebuild seconds.

## Install

User-local symlinks in `~/.local/bin/`. No sudo, easy to undo.

### `~/.local/bin` on PATH

```sh
case ":$PATH:" in
  *":$HOME/.local/bin:"*) : ;;
  *) echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc ;;   # ~/.bashrc for bash; fish: fish_add_path
esac
```

Then `exec zsh` (or open a new terminal).

### Symlinks (auto-detect `zz` alias)

```sh
mkdir -p "$HOME/.local/bin"

ln -sf "$ZZ_HOME/zz-drop/target/release/zz-drop" "$HOME/.local/bin/zz-drop"
ln -sf "$ZZ_HOME/zz-drop-tui/target/release/zz-drop-tui" "$HOME/.local/bin/zz-drop-tui" 2>/dev/null

if existing="$(command -v zz 2>/dev/null)"; then
    echo "note: 'zz' is already on PATH at $existing; short alias not installed"
    echo "      use 'zz-drop' instead, or remove the existing 'zz' first"
else
    ln -sf "$HOME/.local/bin/zz-drop" "$HOME/.local/bin/zz"
fi
```

Idempotent: re-run after every rebuild.

Without `zz-drop-tui`, everything works except `zz c`.

## Shell completion

The completion script is small (~30 lines) and dumb — the
brain lives in the binary, so rebuilding `zz` is what updates
the suggestions. Full setup, scoping rules and styling are in
[`docs/sacs.md`](./sacs.md). Bare-minimum install:

```sh
mkdir -p ~/.zfunc && zz --completions zsh > ~/.zfunc/_zz                    # zsh
zz --completions bash > ~/.local/share/bash-completion/completions/zz       # bash
zz --completions fish > ~/.config/fish/completions/zz.fish                  # fish
```

Then add the recommended rc block from
[`docs/sacs.md`](./sacs.md) (zsh styling, scoped to `zz` only —
no global TAB rebind). The download-glob wrapper (`zz d 'Q*'`
without quotes) is in the same doc.

## Verify

```sh
zz --help               # command summary
zz f                    # doctor: state probe, missing pieces
zz <TAB>                # dropdown should show verbs and atomics
```

## Update

```sh
for d in zz-drop-core zz-drop zz-drop-tui; do
    [ -d "$ZZ_HOME/$d" ] && ( cd "$ZZ_HOME/$d" && git pull --ff-only )
done
( cd "$ZZ_HOME/zz-drop"     && cargo build --release )
( cd "$ZZ_HOME/zz-drop-tui" && cargo build --release ) 2>/dev/null

zz --completions zsh > ~/.zfunc/_zz
rm -f ~/.zcompdump*
```

Symlinks already point at the rebuilt binaries.

## Uninstall

```sh
rm -f "$HOME/.local/bin/"{zz,zz-drop,zz-drop-tui}
rm -f ~/.zfunc/_zz
rm -f ~/.local/share/bash-completion/completions/zz
rm -f ~/.config/fish/completions/zz.fish
# Then remove the zsh block you added to ~/.zshrc by hand.
# Source: rm -rf "$ZZ_HOME/zz-drop" "$ZZ_HOME/zz-drop-core" "$ZZ_HOME/zz-drop-tui"
```

## Troubleshooting

| Symptom                                            | Cause                                                       | Fix                                                                                  |
|----------------------------------------------------|-------------------------------------------------------------|--------------------------------------------------------------------------------------|
| `cargo: command not found`                         | Rust not installed                                          | [rustup.rs](https://rustup.rs/), open new shell                                      |
| `error: linker 'cc' not found` (Linux)             | C toolchain / OpenSSL headers missing                       | `apt install build-essential pkg-config libssl-dev` (or distro equivalent)           |
| `failed to read … zz-drop-core/Cargo.toml`         | Repos not siblings                                          | Re-clone all three under the same `$ZZ_HOME`                                         |
| `zz: command not found` after install              | `~/.local/bin` not on `$PATH`                               | See "`~/.local/bin` on PATH" above                                                   |
| `zz <TAB>` shows nothing                           | Completion script not loaded                                | zsh: ensure `fpath` includes `~/.zfunc` and `compinit` ran                           |
| `zz <TAB>` lists but arrows do nothing             | `menu-select` widget not registered (stock macOS zsh)       | Add `zle -C menu-select .menu-select _main_complete` — see [`sacs.md`](./sacs.md)    |
| `zsh: no matches found: Q*` on `zz d Q*`           | zsh's local glob aborts before the binary                   | Quote (`zz d 'Q*'`) or install the `zz()` wrapper — see [`sacs.md`](./sacs.md)       |
| `zz c` says tui binary not found                   | `zz-drop-tui` not built / not on PATH                       | Clone, build and symlink `zz-drop-tui`                                               |
| `agent locked` everywhere                          | Profile not decrypted in RAM                                | `zz z` to unlock; first-time setup uses `zz c`                                       |
