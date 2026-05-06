#compdef zz zz-drop
# zz-drop zsh completion (SACS)
# install: zz --completions zsh > ~/.zfunc/_zz   (and add ~/.zfunc to fpath)
# marker: zz-drop:sacs-zsh:v1
#
# The script is intentionally dumb — the brain lives in
# `zz __complete`. Any time the grammar changes, rebuild zz;
# this file does not.

_zz_complete() {
    # Don't leak the operator's debug state through TAB. If
    # xtrace/verbose are on in the surrounding shell (`set -x`,
    # `setopt xtrace`, …) zsh would echo every internal
    # assignment of this function above the dropdown. Scope the
    # silence to this function via `localoptions`, and redirect
    # stderr on this very statement so the `setopt` line itself
    # is also not traced.
    { setopt localoptions noxtrace noverbose } 2>/dev/null

    local bin
    if (( $+commands[zz] )); then
        bin=zz
    elif (( $+commands[zz-drop] )); then
        bin=zz-drop
    else
        return 0
    fi

    # Tokens after the program name. When the cursor is past the
    # last word (post-space), zsh leaves $words short by one and
    # $CURRENT points beyond. Append an empty trailing token so
    # SACS can distinguish "verb still being typed" (no trailing)
    # from "verb locked, second positional open" (empty trailing
    # → remote files for `zz d `, etc.).
    local -a args
    args=("${words[@]:1}")
    if (( CURRENT > ${#words} )); then
        args+=("")
    fi

    # Distribute candidates by NDJSON `kind` into per-group
    # arrays. Each group is rendered as a separate `_describe -t`
    # tag; the user's `zstyle group-name` (scoped to `:zz:`)
    # turns those tags into headed sections. Without any zstyle
    # the lot still renders as a flat list — backwards compatible.
    # All locals declared up-front. Declaring `local entry` inside
    # the loop body re-declares an already-defined local on every
    # iteration, and zsh prints the current value to stdout each
    # time — that landed inside the completion buffer as bogus
    # `entry='…'` lines above the dropdown.
    local -a verbs atomics remote_files remote_dirs local_files local_dirs help_entries
    local line value description kind entry
    while IFS= read -r line; do
        value=$(printf '%s' "$line" | sed -n 's/.*"value":[[:space:]]*"\([^"]*\)".*/\1/p')
        description=$(printf '%s' "$line" | sed -n 's/.*"description":[[:space:]]*"\([^"]*\)".*/\1/p')
        kind=$(printf '%s' "$line" | sed -n 's/.*"kind":[[:space:]]*"\([^"]*\)".*/\1/p')

        # Footer is a UX hint emitted by SACS, not a real
        # candidate — skip.
        [[ "$kind" == "footer" ]] && continue
        [[ -z "$value" ]] && continue

        if [[ -n "$description" ]]; then
            entry="${value}:${description}"
        else
            entry="${value}"
        fi

        case "$kind" in
            verb)        verbs+=("$entry") ;;
            atomic)      atomics+=("$entry") ;;
            file_remote) remote_files+=("$entry") ;;
            dir_remote)  remote_dirs+=("$entry") ;;
            file_local)  local_files+=("$entry") ;;
            dir_local)   local_dirs+=("$entry") ;;
            help)        help_entries+=("$entry") ;;
            *)           verbs+=("$entry") ;;
        esac
    done < <("$bin" __complete "${args[@]}" 2>/dev/null)

    (( ${#remote_files} )) && _describe -t remote-files 'remote file'    remote_files
    (( ${#remote_dirs}  )) && _describe -t remote-dirs  'remote dir'     remote_dirs
    (( ${#local_files}  )) && _describe -t local-files  'local file'     local_files
    (( ${#local_dirs}   )) && _describe -t local-dirs   'local dir'      local_dirs
    (( ${#verbs}        )) && _describe -t verbs        'verb'           verbs
    (( ${#atomics}      )) && _describe -t atomics      'atomic command' atomics
    (( ${#help_entries} )) && _describe -t help         'help'           help_entries
}

_zz_complete "$@"
