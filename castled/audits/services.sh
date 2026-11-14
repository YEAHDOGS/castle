#!/usr/bin/env bash
# services.sh -- services running right now (bash twin of phoenix sys-info/list/services.ps1).
# Read-only.
set -u
printf 'SERVICE\tSTATE\tWHAT IT IS\n'
command -v systemctl >/dev/null 2>&1 || { printf -- '-\t-\tThis box does not use systemd\n'; exit 0; }
systemctl list-units --type=service --state=running --no-legend --no-pager --plain 2>/dev/null | awk -v OFS='\t' '
{
    about = ""
    for (i = 5; i <= NF; i++) about = about (i > 5 ? " " : "") $i
    print $1, $4, about
}'
