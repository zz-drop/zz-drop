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
}

complete -F _zz_complete zz
complete -F _zz_complete zz-drop
