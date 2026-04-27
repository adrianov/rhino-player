#!/usr/bin/env sh
set -eu

limit="${RHINO_MODULE_LINE_LIMIT:-1000}"
tmp="${TMPDIR:-/tmp}/rhino-module-lines.$$"
trap 'rm -f "$tmp"' EXIT

git ls-files --cached --others --exclude-standard 'src/*.rs' 'src/**/*.rs' | while IFS= read -r file; do
    lines="$(wc -l < "$file" | tr -d ' ')"
    if [ "$lines" -gt "$limit" ]; then
        printf '%s:%s exceeds %s lines\n' "$file" "$lines" "$limit"
        printf '1\n' > "$tmp"
    fi
done

test ! -s "$tmp"
