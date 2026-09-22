#!/usr/bin/env bash
set -euo pipefail

readonly max_lines=600
readonly baseline_file="scripts/source-size-baseline.txt"
failed=0

if [[ ! -f "$baseline_file" ]]; then
    printf 'source-size baseline is missing: %s\n' "$baseline_file" >&2
    exit 1
fi

while IFS= read -r -d '' file; do
    if [[ ! -f "$file" ]]; then
        continue
    fi
    relative_file="${file#./}"
    if grep -Fqx -- "$relative_file" "$baseline_file"; then
        continue
    fi
    lines=$(wc -l < "$file")
    if (( lines > max_lines )); then
        printf '%s: %d lines (max %d)\n' "$file" "$lines" "$max_lines" >&2
        failed=1
    fi
done < <(git diff --cached --name-only --diff-filter=ACMR -z -- '*.rs')

exit "$failed"
