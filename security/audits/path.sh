#!/usr/bin/env bash
# path.sh -- folders searched for programs (bash twin of phoenix sys-info/list/path.ps1).
# A folder here that other accounts can write to lets them plant a program that runs in
# place of the real one. Read-only.
set -u
printf 'FOLDER\tEXISTS\tOWNER\tOTHERS CAN WRITE\n'
printf '%s\n' "$PATH" | tr ':' '\n' | awk 'NF && !seen[$0]++' | while read -r dir; do
    if [ -d "$dir" ]; then
        mode="$(stat -c '%A' "$dir" 2>/dev/null)"
        owner="$(stat -c '%U' "$dir" 2>/dev/null)"
        writable=no
        case "$mode" in ?????w????|????????w?) writable=YES ;; esac
        printf '%s\tyes\t%s\t%s\n' "$dir" "${owner:--}" "$writable"
    else
        printf '%s\tno\t-\t-\n' "$dir"
    fi
done
