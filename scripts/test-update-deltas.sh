#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/ramag-update-deltas.XXXXXX")
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/previous" "$test_dir/release"
base_name=Ramag-1.0.0-linux-amd64.deb
target_name=Ramag-1.1.0-linux-amd64.deb
head -c 1048576 /dev/zero >"$test_dir/previous/$base_name"
cp "$test_dir/previous/$base_name" "$test_dir/release/$target_name"
printf 'updated' | dd of="$test_dir/release/$target_name" bs=1 seek=5000 conv=notrunc status=none
base_hash=$(sha256sum "$test_dir/previous/$base_name" | cut -d ' ' -f 1)
jq -n --arg name "$base_name" --arg sha "$base_hash" \
    '{schema_version:1,version:"1.0.0",tag_name:"v1.0.0",assets:[{name:$name,size:1048576,sha256:$sha}]}' \
    >"$test_dir/previous/update-manifest.json"
jq -n --arg name "$target_name" '[{name:$name,size:1048576,sha256:"unused"}]' >"$test_dir/assets.json"
(cd "$test_dir/release" && sha256sum "$target_name" >SHA256SUMS.txt)
bash "$script_dir/generate-update-deltas.sh" v1.0.0 "$test_dir/previous" "$test_dir/release" "$test_dir/assets.json"
jq -e '.[0].delta.from_version == "1.0.0" and .[0].delta.patch_size < 1048576' "$test_dir/assets.json" >/dev/null
(cd "$test_dir/release" && sha256sum --check SHA256SUMS.txt)
patch=$(jq -r '.[0].delta.patch_name' "$test_dir/assets.json")
bspatch "$test_dir/previous/$base_name" "$test_dir/rebuilt" "$test_dir/release/$patch"
cmp "$test_dir/rebuilt" "$test_dir/release/$target_name"

# 基线文件被改动时，不生成补丁，也不把差分信息写入清单。
printf 'corrupted' >"$test_dir/previous/$base_name"
jq -n --arg name "$target_name" '[{name:$name,size:1048576,sha256:"unused"}]' >"$test_dir/assets.json"
bash "$script_dir/generate-update-deltas.sh" v1.0.0 "$test_dir/previous" "$test_dir/release" "$test_dir/assets.json"
jq -e '.[0].delta == null' "$test_dir/assets.json" >/dev/null
echo 'Update delta release tests passed.'
