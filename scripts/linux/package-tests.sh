#!/usr/bin/env bash
# Linux 打包纯逻辑回归测试，不编译应用或下载 AppImage 工具。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# shellcheck source=scripts/linux/release-lib.sh
source "$SCRIPT_DIR/release-lib.sh"
# shellcheck source=scripts/package-metadata.sh
source "$SCRIPT_DIR/../package-metadata.sh"

assert_equal() {
    local expected="$1" actual="$2" description="$3"
    if [[ "$actual" != "$expected" ]]; then
        echo "$description: expected '$expected', got '$actual'." >&2
        exit 1
    fi
}

assert_fails() {
    local description="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        echo "$description: expected failure." >&2
        exit 1
    fi
}

assert_equal "Ramag-1.2.3-linux-amd64.deb" \
    "$(linux_get_deb_asset_name "1.2.3")" "Debian asset name"
assert_equal "Ramag-1.2.3-linux-x86_64.AppImage" \
    "$(linux_get_appimage_asset_name "1.2.3")" "AppImage asset name"
assert_fails "Invalid Debian version" linux_get_deb_asset_name "1.2"
assert_fails "Invalid AppImage version" linux_get_appimage_asset_name "next"

assert_equal "octocat" \
    "$(RAMAG_PACKAGE_PUBLISHER=octocat ramag_package_get_author "$REPO_DIR")" \
    "GitHub repository owner override"
(
    unset RAMAG_PACKAGE_PUBLISHER
    cargo_author="$(cargo metadata --locked --no-deps --format-version 1 \
        | jq -er '.packages[] | select(.name == "ramag-bin") | .authors[0]')"
    assert_equal "$cargo_author" "$(ramag_package_get_author "$REPO_DIR")" \
        "Cargo author fallback"
)
RAMAG_PACKAGE_PUBLISHER="bad owner" assert_fails \
    "Invalid GitHub package author" ramag_package_get_author "$REPO_DIR"

author="$(RAMAG_PACKAGE_PUBLISHER=octocat ramag_package_get_author "$REPO_DIR")"
control="$(sed \
    -e "s/@VERSION@/1.2.3/g" \
    -e "s/@INSTALLED_SIZE@/100/g" \
    -e "s/@PACKAGE_AUTHOR@/$author/g" \
    -e "s/@PACKAGE_AUTHOR_EMAIL@/${author}@users.noreply.github.com/g" \
    "$SCRIPT_DIR/debian-control.in")"
grep -Fxq "Maintainer: octocat <octocat@users.noreply.github.com>" <<<"$control"

(
    export GITHUB_REF_TYPE="tag" GITHUB_REF_NAME="v1.2.3"
    linux_assert_tag_matches_version "1.2.3"
)
# shellcheck disable=SC2016
assert_fails "Mismatched release tag" bash -c '
    source "$1"
    export GITHUB_REF_TYPE=tag GITHUB_REF_NAME=v1.2.4
    linux_assert_tag_matches_version 1.2.3
' _ "$SCRIPT_DIR/release-lib.sh"

version="$(linux_get_app_version "$REPO_DIR")"
linux_is_supported_semver "$version"

desktop_file="$SCRIPT_DIR/com.ramag.Ramag.desktop"
grep -Fxq 'Exec=ramag' "$desktop_file"
grep -Fxq 'Icon=com.ramag.Ramag' "$desktop_file"
grep -Fxq 'StartupWMClass=com.ramag.Ramag' "$desktop_file"

echo "Linux packaging tests passed."
