#!/usr/bin/env bash
set -euo pipefail

tag="${1:?release tag required}"
repository="${2:?repository required}"
output="${3:?GitHub Actions output path required}"

if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.+-]+)?$ ]] ||
   [[ "$(git cat-file -t "refs/tags/$tag")" != tag ]]; then
    echo "Release requires an annotated version tag: $tag" >&2
    exit 1
fi
commit=$(git rev-parse "refs/tags/$tag^{commit}")

# Only the repository's successful main runs can supply release packages.
run_ids=$(gh api --method GET "repos/$repository/actions/workflows/desktop-ci.yml/runs" \
    -f branch=main -f head_sha="$commit" -f status=success -F per_page=100 --paginate |
    jq -r --arg commit "$commit" --arg repository "$repository" '
        .workflow_runs[] |
        select(.head_sha == $commit and .head_branch == "main"
            and .head_repository.full_name == $repository
            and .conclusion == "success"
            and (.event == "push" or .event == "workflow_dispatch")) | .id')
expected=$(jq -nc --arg sha "$commit" '["linux", "macos", "windows"] |
    map("ramag-" + . + "-" + $sha)')

while IFS= read -r run_id; do
    [[ -n "$run_id" ]] || continue
    artifacts=$(gh api "repos/$repository/actions/runs/$run_id/artifacts")
    if jq -e --argjson expected "$expected" '
        [.artifacts[] | select(.expired == false and .size_in_bytes > 0) | .name] as $available |
        all($expected[]; . as $name | any($available[]; . == $name))
    ' <<<"$artifacts" >/dev/null; then
        printf 'run_id=%s\ncommit=%s\n' "$run_id" "$commit" >>"$output"
        printf 'Using Desktop CI run %s for %s (%s).\n' "$run_id" "$tag" "$commit"
        exit 0
    fi
done <<<"$run_ids"

echo "No successful Desktop CI run has all three packages for $commit." >&2
echo "Run or rerun Desktop CI for this exact main commit, then retry release $tag." >&2
exit 1
