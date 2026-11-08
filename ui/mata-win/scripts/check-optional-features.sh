#!/usr/bin/env bash
set -euo pipefail

# Run from the parent workspace root, not from inside mata-win.
# On Linux, a window backend is mandatory, even when all Mata integrations are off.
backend=""
if [[ "$(uname -s)" == "Linux" ]]; then
  backend="x11"
fi

check() {
  local label="$1"; shift
  printf '\n--- %s ---\n' "$label"
  cargo check -p mata-win --lib --no-default-features "$@"
}

if [[ -n "$backend" ]]; then
  check "no Mata integrations" --features "$backend"
  for flag in fonts audio image video; do
    check "only $flag" --features "$backend,$flag"
  done
  check "video without audio or image" --features "$backend,video"
  check "video and audio" --features "$backend,video,audio"
  check "all Mata integrations" --features "$backend,fonts,audio,image,video"
else
  check "no Mata integrations"
  for flag in fonts audio image video; do
    check "only $flag" --features "$flag"
  done
  check "video and audio" --features video,audio
  check "all Mata integrations" --features fonts,audio,image,video
fi
# Check the examples whose integrations are enabled (legacy examples may
# depend on old MataWin APIs and are outside this feature matrix).
cargo check -p mata-win --example widget_media --features audio
cargo check -p mata-win --example widget_image --features image
cargo check -p mata-win --example image_gallery --features image,fonts
cargo check -p mata-win --example video_widget --features video
