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

    # Files/verbs/atomics use `_describe` so the description string
    # appears next to each row and the section header inherits the
    # operator's `:descriptions` format. Directories use `compadd`
    # directly: the values already end with `/`, and `-S ''`
    # suppresses zsh's default trailing-space suffix so a follow-up
    # TAB can descend into the directory without a backspace.
    # Trying to pass `-- -S ''` through `_describe` triggered
    # `compdescribe: invalid argument` in some zsh 5.x builds, so
    # the dir branches are split off.
    (( ${#remote_files} )) && _describe -t remote-files 'remote file'    remote_files
    (( ${#local_files}  )) && _describe -t local-files  'local file'     local_files
    (( ${#verbs}        )) && _describe -t verbs        'verb'           verbs
    (( ${#atomics}      )) && _describe -t atomics      'atomic command' atomics
    (( ${#help_entries} )) && _describe -t help         'help'           help_entries

    # `_description` runs the operator's `:descriptions` zstyle
    # against the section header (so "[local dir]" gets the cyan
    # bracket format defined for the (zz|zz-drop) context, same as
    # the headers `_describe` produces). `compadd -S ''` then
    # suppresses zsh's default trailing-space suffix so a follow-up
    # TAB can descend into the directory without a backspace.
    if (( ${#local_dirs} )); then
        local -a _local_dir_values=() _expl
        local _d
        for _d in "${local_dirs[@]}"; do
            _local_dir_values+=("${_d%%:*}")
        done
        _description local-dirs _expl 'local dir'
        compadd "${_expl[@]}" -S '' -a _local_dir_values
    fi
    if (( ${#remote_dirs} )); then
        local -a _remote_dir_values=() _expl
        local _d
        for _d in "${remote_dirs[@]}"; do
            _remote_dir_values+=("${_d%%:*}")
        done
        _description remote-dirs _expl 'remote dir'
        compadd "${_expl[@]}" -S '' -a _remote_dir_values
    fi
}

_zz_complete "$@"
