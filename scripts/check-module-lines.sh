#!/usr/bin/env sh
# Advisory check: lists Rust sources under src/ longer than $RHINO_MODULE_LINE_LIMIT (default 300).
# Always exits 0. Refactor when convenient — not a merge gate.

set -eu

limit="${RHINO_MODULE_LINE_LIMIT:-300}"
tmp="${TMPDIR:-/tmp}/rhino-module-lines-sort.$$"
trap 'rm -f "$tmp"' EXIT
: >"$tmp"

git ls-files --cached --others --exclude-standard 'src/*.rs' 'src/**/*.rs' | while IFS= read -r file; do
    lines="$(wc -l < "$file" | tr -d ' ')"
    if [ "$lines" -gt "$limit" ]; then
        printf '%s %s\n' "$lines" "$file" >>"$tmp"
    fi
done

if [ ! -s "$tmp" ]; then
    exit 0
fi

# Numeric line count, descending (longest module first in the report)
sort -k1,1nr "$tmp" | while read -r lines file; do
    printf 'module-lines: warning: %s has %s lines (soft limit %s)\n' "$file" "$lines" "$limit" >&2
done

printf '%s\n' "module-lines: when several modules are over the limit, refactor one file at a time, starting with the longest." >&2

exit 0
