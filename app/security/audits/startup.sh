#!/usr/bin/env bash
# startup.sh -- everything set to start on its own (bash twin of phoenix sys-info/list/startup.ps1).
# Read-only. Covers enabled systemd units, cron's @reboot lines, rc.local and desktop autostart.
set -u
TAB="$(printf '\t')"
printf 'WHERE\tNAME\tRUNS\n'

if command -v systemctl >/dev/null 2>&1; then
    systemctl list-unit-files --state=enabled --no-legend --no-pager --plain 2>/dev/null | while read -r unit _; do
        [ -n "$unit" ] || continue
        runs="$(systemctl show -p ExecStart --value "$unit" 2>/dev/null | sed -n 's/.*argv\[\]=\([^;]*\);.*/\1/p' | head -n 1)"
        printf 'systemd\t%s\t%s\n' "$unit" "${runs:--}"
    done
fi

for file in /etc/crontab /etc/cron.d/* /var/spool/cron/crontabs/* /var/spool/cron/*; do
    [ -f "$file" ] && [ -r "$file" ] || continue
    grep -E '^[[:space:]]*@reboot' "$file" 2>/dev/null | while read -r line; do
        printf 'cron @reboot\t%s\t%s\n' "$file" "${line//$TAB/ }"
    done
done

if [ -r /etc/rc.local ]; then
    grep -vE '^[[:space:]]*(#|$|exit 0)' /etc/rc.local 2>/dev/null | while read -r line; do
        printf 'rc.local\t/etc/rc.local\t%s\n' "${line//$TAB/ }"
    done
fi

for file in /etc/xdg/autostart/*.desktop /home/*/.config/autostart/*.desktop /root/.config/autostart/*.desktop; do
    [ -r "$file" ] || continue
    runs="$(sed -n 's/^Exec=//p' "$file" | head -n 1)"
    printf 'desktop autostart\t%s\t%s\n' "$file" "${runs:--}"
done
