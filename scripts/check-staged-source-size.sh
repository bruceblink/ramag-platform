#!/usr/bin/env bash
set -euo pipefail

readonly max_lines=600
failed=0

while IFS= read -r -d '' file; do
    if [[ ! -f "$file" ]]; then
        continue
    fi
    lines=$(wc -l < "$file")
    if (( lines > max_lines )); then
        printf '%s: %d lines (max %d)\n' "$file" "$lines" "$max_lines" >&2
        failed=1
    fi
done < <(git diff --cached --name-only --diff-filter=ACMR -z -- '*.rs')

exit "$failed"
