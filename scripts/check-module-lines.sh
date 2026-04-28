#!/usr/bin/env sh
# Soft limit: warn on Rust sources under src/ longer than $RHINO_MODULE_LINE_LIMIT (default 300).
# Hard limit: exit 1 if any file exceeds $RHINO_MODULE_LINE_HARD (default 995 = longest module
# over soft at check introduction). Lower the default when that module shrinks or splits.
#
# Override limits for local experiments: RHINO_MODULE_LINE_LIMIT, RHINO_MODULE_LINE_HARD.

set -eu

soft="${RHINO_MODULE_LINE_LIMIT:-300}"
hard="${RHINO_MODULE_LINE_HARD:-995}"
tmpd="${TMPDIR:-/tmp}/rhino-module-lines.$$"
trap 'rm -rf "$tmpd"' EXIT
mkdir "$tmpd"
all="$tmpd/all"
sortf="$tmpd/sort"
: >"$all"

git ls-files --cached --others --exclude-standard 'src/*.rs' 'src/**/*.rs' | while IFS= read -r file; do
    lines="$(wc -l < "$file" | tr -d ' ')"
    printf '%s %s\n' "$lines" "$file" >>"$all"
done

sort -k1,1nr "$all" >"$sortf"

any_soft=0
hard_fail=0

while read -r lines file; do
    if [ "$lines" -gt "$hard" ]; then
        printf 'module-lines: error: %s has %s lines (hard limit %s; soft %s)\n' "$file" "$lines" "$hard" "$soft" >&2
        hard_fail=1
    elif [ "$lines" -gt "$soft" ]; then
        printf 'module-lines: warning: %s has %s lines (soft limit %s)\n' "$file" "$lines" "$soft" >&2
        any_soft=1
    fi
done <"$sortf"

if [ "$hard_fail" -eq 1 ]; then
    printf '%s\n' "module-lines: no module may exceed the hard limit — refactor or raise RHINO_MODULE_LINE_HARD after intentional review." >&2
    exit 1
fi

if [ "$any_soft" -eq 1 ]; then
    printf '%s\n' "module-lines: when several modules are over the soft limit, refactor one file at a time, starting with the longest." >&2
fi

exit 0
