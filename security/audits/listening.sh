#!/usr/bin/env bash
# listening.sh -- what is listening for connections (the listener half of phoenix
# scripts/tools/netmon.sh). Read-only. Process names need root; without it they show "-".
set -u
printf 'PROTOCOL\tADDRESS\tPORT\tREACHABLE FROM\tPROCESS\n'
command -v ss >/dev/null 2>&1 || { printf -- '-\t-\t-\t-\tss (iproute2) is not installed\n'; exit 0; }
ss -H -tulnp 2>/dev/null | awk -v OFS='\t' '
{
    proto = $1
    addr = $5
    n = split(addr, parts, ":")
    port = parts[n]
    sub(/:[^:]*$/, "", addr)
    gsub(/^\[|\]$/, "", addr)
    reach = "other machines"
    if (addr ~ /^127\./ || addr == "::1") reach = "this box only"
    proc = "-"
    line = $0
    if (sub(/.*users:\(\("/, "", line)) { sub(/".*/, "", line); proc = line }
    print proto, addr, port, reach, proc
}' | sort -u -t "$(printf '\t')" -k4,4 -k3,3n
