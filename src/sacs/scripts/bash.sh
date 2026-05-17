# zz-drop bash completion (SACS)
# install: zz --completions bash | source
# marker: zz-drop:sacs-bash:v1
#
# The script is intentionally dumb — the brain lives in
# `zz __complete`. Any time the grammar changes, rebuild zz;
# this file does not.

_zz_complete() {
    # Prefer `zz` on PATH; fall back to `zz-drop` if the symlink
    # is missing. If neither is reachable, give up silently.
    local bin
    if command -v zz >/dev/null 2>&1; then
        bin=zz
    elif command -v zz-drop >/dev/null 2>&1; then
        bin=zz-drop
    else
        return 0
    fi

    # Tokens after the program name, in order. `zz __complete`
    # decides ranking and content; we just relay the cursor
    # context.
    local -a cmd_args
    cmd_args=("${COMP_WORDS[@]:1}")

    # NDJSON output: one object per line. Extract `value` with
    # a primitive sed pattern. Both ends of the wire are owned
    # by zz — the value is sanitised at emit time so it never
    # contains an unescaped quote.
    COMPREPLY=()
    local line value
    while IFS= read -r line; do
        value=$(printf '%s' "$line" | sed -n 's/.*"value":[[:space:]]*"\([^"]*\)".*/\1/p')
        if [ -n "$value" ]; then
            COMPREPLY+=("$value")
        fi
    done < <("$bin" __complete "${cmd_args[@]}" 2>/dev/null)

    # Match the zsh script's `compadd -S ''` behaviour: when every
    # candidate is a directory (value ends with `/`), suppress
    # bash's auto-appended trailing space so a follow-up TAB can
    # descend into the directory without a backspace.
    if [ ${#COMPREPLY[@]} -gt 0 ]; then
        local _zz_c _zz_all_dirs=1
        for _zz_c in "${COMPREPLY[@]}"; do
            case "$_zz_c" in
                */) ;;
                *) _zz_all_dirs=0; break ;;
            esac
        done
        if [ $_zz_all_dirs -eq 1 ]; then
            compopt -o nospace 2>/dev/null
        fi
        unset _zz_c _zz_all_dirs
    fi
}

complete -F _zz_complete zz
complete -F _zz_complete zz-drop
