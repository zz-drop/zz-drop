# zz-drop fish completion (SACS)
# install: zz --completions fish > ~/.config/fish/completions/zz.fish
# marker: zz-drop:sacs-fish:v1
#
# The script is intentionally dumb — the brain lives in
# `zz __complete`. Any time the grammar changes, rebuild zz;
# this file does not.

function __zz_complete
    set -l bin
    if command -q zz
        set bin zz
    else if command -q zz-drop
        set bin zz-drop
    else
        return 0
    end

    # `commandline -opc` is the tokenised buffer up to but not
    # including the cursor word; index 1 is the program name.
    # Always append the cursor word — including the empty string
    # post-space — so SACS can distinguish "completing the verb
    # `d`" (no trailing token) from "verb `d` is locked, complete
    # the second positional" (empty trailing token → remote files).
    set -l tokens (commandline -opc)[2..]
    set -a tokens (commandline -ct)

    $bin __complete $tokens 2>/dev/null | while read -l line
        set -l value (string match -r '"value":\s*"([^"]*)"' -- $line)[2]
        set -l description (string match -r '"description":\s*"([^"]*)"' -- $line)[2]
        if test -n "$value"
            if test -n "$description"
                printf '%s\t%s\n' $value $description
            else
                printf '%s\n' $value
            end
        end
    end
end

complete -c zz -f -a '(__zz_complete)'
complete -c zz-drop -f -a '(__zz_complete)'
