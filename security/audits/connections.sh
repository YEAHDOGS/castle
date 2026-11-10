#!/usr/bin/env bash
# connections.sh -- established TCP connections, outside world first (from phoenix
# scripts/tools/netmon.sh). Read-only. Process names need root; without it they show "-".
set -u
printf 'GOES TO\tREMOTE ADDRESS\tPORT\tPROCESS\n'
command -v ss >/dev/null 2>&1 || { printf -- '-\t-\t-\tss (iproute2) is not installed\n'; exit 0; }

# classify_ip <ip> -> "local network" for private, loopback and link-local addresses.
classify_ip() {
    local ip="${1%%\%*}"
    case "$ip" in
        127.*|::1|0.0.0.0|::|10.*|192.168.*|169.254.*) echo 'local network'; return ;;
        172.1[6-9].*|172.2[0-9].*|172.3[0-1].*)         echo 'local network'; return ;;
        fc*|fd*|fe80:*)                                  echo 'local network'; return ;;
    esac
    echo 'outside world'
}

ss -H -tupn state established 2>/dev/null | awk '
{
    peer = $5
    n = split(peer, parts, ":")
    port = parts[n]
    sub(/:[^:]*$/, "", peer)
    gsub(/^\[|\]$/, "", peer)
    proc = "-"
    line = $0
    if (sub(/.*users:\(\("/, "", line)) { sub(/".*/, "", line); proc = line }
    print peer, port, proc
}' | while read -r ip port proc; do
    [ -n "$ip" ] || continue
    printf '%s\t%s\t%s\t%s\n' "$(classify_ip "$ip")" "$ip" "$port" "$proc"
done | sort -t "$(printf '\t')" -k1,1r -k2,2
