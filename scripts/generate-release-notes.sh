#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "usage: $0 <release-ref> <github-repository> [output-path]" >&2
  exit 2
fi

release_ref="$1"
repository="$2"
output_path="${3:-release-notes.md}"
git rev-parse --verify "${release_ref}^{commit}" >/dev/null

release_tag="$release_ref"
if [[ "$release_tag" != v* ]]; then
  release_tag="$(git describe --exact-match --tags "$release_ref" 2>/dev/null || printf '%s' "$release_ref")"
fi
version="${release_tag#v}"
release_date="$(git show -s --format='%cs' "${release_ref}^{commit}")"

previous_tag=""
while IFS= read -r candidate; do
  if [[ "$candidate" != "$release_tag" && "$candidate" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]]; then
    previous_tag="$candidate"
    break
  fi
done < <(git tag --sort=-version:refname --merged "$release_ref")

release_range="$release_ref"
if [[ -n "$previous_tag" ]]; then
  release_range="${previous_tag}..${release_ref}"
else
  # The first independent release starts after the latest historical release marker.
  baseline="$(git log --all --format='%H' --grep='^发布：Ramag v[0-9]' -n 1 || true)"
  if [[ -n "$baseline" ]]; then
    release_range="${baseline}..${release_ref}"
  fi
fi

temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT

declare -A section_files=(
  [features]="$temp_dir/features"
  [fixes]="$temp_dir/fixes"
  [security]="$temp_dir/security"
  [performance]="$temp_dir/performance"
  [maintenance]="$temp_dir/maintenance"
  [other]="$temp_dir/other"
)
for file in "${section_files[@]}"; do
  : >"$file"
done

# Map each Conventional Commit type to the shared bilingual release section.
while IFS=$'\t' read -r commit subject; do
  [[ -n "$commit" && -n "$subject" ]] || continue
  commit_type="${subject%%:*}"
  if [[ "$commit_type" == "$subject" ]]; then
    section=other
  else
    commit_type="${commit_type%%(*}"
    case "${commit_type,,}" in
      feat) section=features ;;
      fix) section=fixes ;;
      security) section=security ;;
      perf) section=performance ;;
      docs|refactor|test|ci|build|chore|revert) section=maintenance ;;
      *) section=other ;;
    esac
  fi
  printf -- '- %s ([%s](https://github.com/%s/commit/%s))\n' \
    "$subject" "${commit:0:7}" "$repository" "$commit" >>"${section_files[$section]}"
done < <(git log --no-merges --format='%H%x09%s' --reverse "$release_range")

notes_file="$temp_dir/release-notes.md"
{
  printf '## [%s] - %s\n\n' "$version" "$release_date"
  if [[ -n "$previous_tag" ]]; then
    printf 'Full Changelog: https://github.com/%s/compare/%s...%s\n\n' \
      "$repository" "$previous_tag" "$release_tag"
  fi

  if [[ -s "${section_files[features]}" ]]; then
    printf '### 🚀 新功能 / Features\n\n'
    cat "${section_files[features]}"
    printf '\n'
  fi
  if [[ -s "${section_files[fixes]}" ]]; then
    printf '### 🐛 问题修复 / Bug Fixes\n\n'
    cat "${section_files[fixes]}"
    printf '\n'
  fi
  if [[ -s "${section_files[security]}" ]]; then
    printf '### 🔒 安全 / Security\n\n'
    cat "${section_files[security]}"
    printf '\n'
  fi
  if [[ -s "${section_files[performance]}" ]]; then
    printf '### ⚡ 性能优化 / Performance\n\n'
    cat "${section_files[performance]}"
    printf '\n'
  fi
  if [[ -s "${section_files[maintenance]}" ]]; then
    printf '### 🧰 维护、文档与测试 / Maintenance, Docs & Tests\n\n'
    cat "${section_files[maintenance]}"
    printf '\n'
  fi
  if [[ -s "${section_files[other]}" ]]; then
    printf '### 📝 其他变更 / Other Changes\n\n'
    cat "${section_files[other]}"
    printf '\n'
  fi
} >"$notes_file"

mv "$notes_file" "$output_path"
