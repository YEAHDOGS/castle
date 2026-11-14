#!/usr/bin/env bash
# tasks.sh -- scheduled tasks (bash twin of phoenix sys-info/list/tasks.ps1). Read-only.
# Covers systemd timers and every crontab this account can read.
set -u
TAB="$(printf '\t')"
printf 'KIND\tNAME\tWHEN\tRUNS\n'

if command -v systemctl >/dev/null 2>&1; then
    systemctl list-unit-files --type=timer --no-legend --no-pager --plain 2>/dev/null | while read -r timer state _; do
        [ -n "$timer" ] || continue
        when="$(systemctl show -p TimersCalendar -p TimersMonotonic --value "$timer" 2>/dev/null | sed -n 's/^{ *\([^;]*\);.*/\1/p' | tr '\n' ' ')"
        runs="$(systemctl show -p Unit --value "$timer" 2>/dev/null)"
        printf 'timer (%s)\t%s\t%s\t%s\n' "$state" "$timer" "${when:--}" "${runs:--}"
    done
fi

for file in /etc/crontab /etc/cron.d/* /var/spool/cron/crontabs/* /var/spool/cron/*; do
    [ -f "$file" ] && [ -r "$file" ] || continue
    grep -vE '^[[:space:]]*(#|$|[A-Za-z_]+=)' "$file" 2>/dev/null | while read -r m h dom mon dow rest; do
        case "$m" in
            @*) when="$m"; runs="$h $dom $mon $dow $rest" ;;
            *)  when="$m $h $dom $mon $dow"; runs="$rest" ;;
        esac
        printf 'cron\t%s\t%s\t%s\n' "$file" "$when" "${runs//$TAB/ }"
    done
done

for dir in /etc/cron.hourly /etc/cron.daily /etc/cron.weekly /etc/cron.monthly; do
    [ -d "$dir" ] || continue
    for file in "$dir"/*; do
        [ -f "$file" ] || continue
        printf 'cron\t%s\t%s\t%s\n' "$file" "${dir##*/cron.}" "$file"
    done
done
