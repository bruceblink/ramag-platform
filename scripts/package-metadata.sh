#!/usr/bin/env bash
# Shared release metadata resolution for Linux and macOS package builders.

ramag_package_get_author() {
    local repo_dir="$1"
    local author="${RAMAG_PACKAGE_PUBLISHER:-}"

    if [[ -z "$author" ]]; then
        local metadata
        metadata="$(cd "$repo_dir" && cargo metadata --locked --no-deps --format-version 1)" || {
            echo "Failed to read Cargo metadata for the package author." >&2
            return 1
        }
        author="$(printf '%s' "$metadata" | jq -er '
            [.packages[] | select(.name == "ramag-bin")]
            | if length == 1 and (.[0].authors | length) > 0
              then .[0].authors[0]
              else error("expected one ramag-bin package with an author")
              end
        ')" || {
            echo "Cargo metadata must contain one ramag-bin package author." >&2
            return 1
        }
    fi

    if [[ ! "$author" =~ ^[A-Za-z0-9]([A-Za-z0-9-]{0,37}[A-Za-z0-9])?$ ]]; then
        echo "Package author must be a GitHub user or organization name." >&2
        return 1
    fi
    printf '%s\n' "$author"
}
