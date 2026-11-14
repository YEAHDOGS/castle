#!/usr/bin/env bash
# users.sh -- every account on the box (bash twin of phoenix sys-info/list/users.ps1).
# Read-only. Output: a header line, then one tab-separated row per account.
set -u
printf 'USER\tUID\tCAN LOG IN\tGROUPS\tSHELL\tHOME\n'
while IFS=: read -r name _ uid _ _ home shell; do
    case "$shell" in
        */nologin|*/false|'') login=no ;;
        *) login=yes ;;
    esac
    groups="$(id -Gn "$name" 2>/dev/null | tr ' ' ',')"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$uid" "$login" "${groups:--}" "${shell:--}" "${home:--}"
done < /etc/passwd | sort -t "$(printf '\t')" -k3,3r -k2,2n
