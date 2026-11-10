#!/usr/bin/env bash
# hosts.sh -- names this box resolves without asking DNS (bash twin of phoenix
# sys-info/list/hosts.ps1). An unexpected entry can silently send a name somewhere else.
# Read-only.
set -u
printf 'ADDRESS\tNAMES\n'
[ -r /etc/hosts ] || exit 0
grep -vE '^[[:space:]]*(#|$)' /etc/hosts | while read -r address names; do
    printf '%s\t%s\n' "$address" "$(printf '%s' "${names%%#*}" | tr -s '\t ' ' ')"
done
