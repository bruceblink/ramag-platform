#!/usr/bin/env bash
set -euo pipefail

# 旧 Release 清单、旧包目录、新包目录和待更新的资产 JSON。
previous_tag="$1"
previous_dir="$2"
release_dir="$3"
assets_file="$4"
previous_manifest="$previous_dir/update-manifest.json"
max_bytes=$((128 * 1024 * 1024))

previous_version=$(jq -er '.version | select(test("^[0-9]+\\.[0-9]+\\.[0-9]+$"))' "$previous_manifest")
[[ "$previous_tag" == "v$previous_version" ]]
[[ "$(jq -r '.tag_name' "$previous_manifest")" == "$previous_tag" ]]
[[ "$(jq -r '.schema_version' "$previous_manifest")" == 1 ]]

while IFS= read -r name; do
    [[ "$name" =~ ^Ramag-([0-9]+\.[0-9]+\.[0-9]+)-[a-zA-Z0-9._-]+$ ]] || continue
    version="${BASH_REMATCH[1]}"
    [[ "$version" != "$previous_version" ]] || continue
    base_name="Ramag-$previous_version-${name#Ramag-"$version"-}"
    base_info=$(jq -ce --arg name "$base_name" '[.assets[] | select(.name == $name)] | if length == 1 then .[0] else empty end' "$previous_manifest") || continue
    base_size=$(jq -er '.size | select(type == "number" and . > 0 and floor == .)' <<<"$base_info") || continue
    base_hash=$(jq -er '.sha256 | select(test("^[0-9a-f]{64}$"))' <<<"$base_info") || continue
    [[ "$base_size" -le "$max_bytes" ]] || continue
    target="$release_dir/$name"
    [[ -f "$target" ]] || continue
    target_size=$(stat -c '%s' "$target")
    [[ "$target_size" -le "$max_bytes" ]] || continue
    base="$previous_dir/$base_name"
    if [[ ! -f "$base" ]] && ! gh release download "$previous_tag" --pattern "$base_name" --dir "$previous_dir"; then
        echo "Skipping delta: cannot download $base_name" >&2
        continue
    fi
    [[ "$(stat -c '%s' "$base")" == "$base_size" ]] || continue
    [[ "$(sha256sum "$base" | cut -d ' ' -f 1)" == "$base_hash" ]] || continue
    patch_name="$name.from-$previous_version.bsdiff"
    patch="$release_dir/$patch_name"
    rebuilt=$(mktemp "${TMPDIR:-/tmp}/ramag-update-delta.XXXXXX")
    if ! timeout 120 bsdiff "$base" "$target" "$patch" ||
       ! timeout 120 bspatch "$base" "$rebuilt" "$patch" ||
       ! cmp -s "$target" "$rebuilt"; then
        echo "Skipping delta: generation or verification failed for $name" >&2
        rm -f -- "$patch" "$rebuilt"
        continue
    fi
    rm -f -- "$rebuilt"
    patch_size=$(stat -c '%s' "$patch")
    if (( patch_size == 0 || patch_size >= target_size )); then
        rm -f -- "$patch"
        continue
    fi
    patch_hash=$(sha256sum "$patch" | cut -d ' ' -f 1)
    delta=$(jq -cn --arg from_version "$previous_version" --arg base_name "$base_name" \
        --argjson base_size "$base_size" --arg base_sha256 "$base_hash" --arg patch_name "$patch_name" \
        --argjson patch_size "$patch_size" --arg patch_sha256 "$patch_hash" \
        '{from_version:$from_version,base_name:$base_name,base_size:$base_size,base_sha256:$base_sha256,patch_name:$patch_name,patch_size:$patch_size,patch_sha256:$patch_sha256}')
    jq -c --arg name "$name" --argjson delta "$delta" \
        'map(if .name == $name then . + {delta:$delta} else . end)' "$assets_file" >"$assets_file.tmp"
    mv -- "$assets_file.tmp" "$assets_file"
    printf '%s  %s\n' "$patch_hash" "$patch_name" >>"$release_dir/SHA256SUMS.txt"
done < <(jq -r '.[].name' "$assets_file")
