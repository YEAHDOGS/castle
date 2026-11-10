#!/usr/bin/env bash
# modules.sh -- code loaded into the kernel (bash twin of phoenix sys-info/list/drivers.ps1).
# Read-only.
set -u
printf 'MODULE\tSIZE\tUSED BY\n'
[ -r /proc/modules ] || { printf -- '-\t-\tThis kernel does not list its modules\n'; exit 0; }
awk -v OFS='\t' '{ used = $4; sub(/,$/, "", used); if (used == "-") used = "nothing"; print $1, $2, used }' /proc/modules | sort
