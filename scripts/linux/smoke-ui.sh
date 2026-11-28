#!/usr/bin/env bash
set -euo pipefail

binary="${1:-target/debug/examples/ui-preview}"
window_timeout="${UI_SMOKE_TIMEOUT_SECONDS:-35}"

if [[ ! -x "$binary" ]]; then
    printf 'UI smoke failed: executable not found or not executable: %s\n' "$binary" >&2
    exit 2
fi
for command_name in timeout xvfb-run; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'UI smoke failed: required command is unavailable: %s\n' "$command_name" >&2
        exit 2
    fi
done
if ! [[ "$window_timeout" =~ ^[1-9][0-9]*$ ]]; then
    printf 'UI smoke failed: UI_SMOKE_TIMEOUT_SECONDS must be a positive integer\n' >&2
    exit 2
fi

printf 'UI smoke: building binary is expected to be a separate CI step\n'
printf 'UI smoke: launching home/dark at 1440x900 under Xvfb (window timeout %ss)\n' "$window_timeout"

# Bound the example itself so xvfb-run can clean up its server after a timeout.
# The outer bound also covers xvfb-run startup and shutdown.
if timeout --kill-after=5s 45s xvfb-run --auto-servernum \
    --server-args='-screen 0 1600x1000x24' \
    timeout --kill-after=5s "${window_timeout}s" \
    env -u WAYLAND_DISPLAY WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe \
    "$binary" --smoke home dark 1440 900; then
    printf 'UI smoke passed: preview window opened and process exited successfully\n'
else
    status=$?
    if [[ "$status" -eq 124 || "$status" -eq 137 ]]; then
        printf 'UI smoke failed: preview or Xvfb exceeded the 35s app / 45s outer time bounds\n' >&2
    else
        printf 'UI smoke failed: command exited with status %s\n' "$status" >&2
    fi
    exit "$status"
fi
