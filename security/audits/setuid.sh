#!/usr/bin/env bash
# setuid.sh -- programs any account can run with their owner's rights. A program here that
# you do not recognise, or one outside the system folders, is worth a look.
# Read-only. Stays on each folder's own filesystem; does not cross mounts.
set -u
printf 'PROGRAM\tRUNS AS\tKIND\tLAST CHANGED\n'
for dir in / /usr /opt /home /var; do
    [ -d "$dir" ] || continue
    find "$dir" -xdev \( -perm -4000 -o -perm -2000 \) -type f -printf '%p\t%u\t%M\t%TY-%Tm-%Td\n' 2>/dev/null
done | sort -u | awk -F '\t' -v OFS='\t' '
{
    kind = "setgid"
    flag = substr($3, 4, 1)
    if (flag == "s" || flag == "S") kind = "setuid"
    print $1, $2, kind, $4
}'
