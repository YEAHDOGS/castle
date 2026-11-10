#!/usr/bin/env bash
# test.sh -- runs every audit the way the box does (fed to bash on standard input) and
# checks each prints a header and rows with the same number of columns.
# Usage: bash security/audits/test.sh [--show]
set -u
here="$(cd "$(dirname "$0")" && pwd)"
show="${1:-}"
failed=0
for script in "$here"/*.sh; do
    name="$(basename "$script" .sh)"
    [ "$name" = test ] && continue
    output="$(LC_ALL=C bash -s < "$script" 2>&1)"
    columns="$(printf '%s\n' "$output" | head -n 1 | awk -F '\t' '{ print NF }')"
    uneven="$(printf '%s\n' "$output" | awk -F '\t' -v want="$columns" 'NF && NF != want' | wc -l)"
    rows="$(( $(printf '%s\n' "$output" | grep -c .) - 1 ))"
    if [ "${columns:-0}" -lt 2 ] || [ "$uneven" -ne 0 ]; then
        echo "FAIL $name: $columns columns, $uneven uneven rows"
        failed=1
    else
        echo "ok   $name: $columns columns, $rows rows"
    fi
    [ "$show" = --show ] && printf '%s\n' "$output" | head -n 6 | cut -c1-160
done
exit "$failed"
