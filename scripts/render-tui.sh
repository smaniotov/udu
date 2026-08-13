#!/usr/bin/env bash
# Render a udu TUI frame in a real (headless) Ghostty and capture it to a PNG.
# This is the visual validation loop: render -> look -> iterate.
#
# Usage: scripts/render-tui.sh [state] [out.png]
#   state: soundpacks (default) | devices | audio | help
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/debug/snapshot"
STATE="${1:-soundpacks}"
OUT="${2:-$ROOT/udu-$(date +%H%M%S).png}"
DISP=":99"

case "$STATE" in
  soundpacks|devices|audio|help|general|about) ;;
  *) echo "unknown state: $STATE (soundpacks|devices|audio|help|general|about)"; exit 1 ;;
esac

if [ ! -x "$BIN" ]; then
  echo "binary not found: $BIN — run: cargo build --bin snapshot"
  exit 1
fi

cleanup() {
  kill "$GT" 2>/dev/null
  kill "$XVFB_PID" 2>/dev/null
  pkill -f "snapshot --live" 2>/dev/null
  pkill -f "ghostty --gtk-single-instance=false -e" 2>/dev/null
}
trap cleanup EXIT

pkill -f "snapshot --live" 2>/dev/null
pkill -f "ghostty --gtk-single-instance=false -e" 2>/dev/null
sleep 1

Xvfb "$DISP" -screen 0 920x660x24 2>/tmp/udu-xvfb.log &
XVFB_PID=$!
sleep 2

DISPLAY="$DISP" GDK_BACKEND=x11 ghostty --gtk-single-instance=false \
  --font-size=11 --window-width=920 --window-height=660 \
  -e "$BIN" --live --state "$STATE" --size-file /tmp/udu-size.txt \
  >/tmp/udu-ghostty.log 2>&1 &
GT=$!
sleep 8

SIZE="$(cat /tmp/udu-size.txt 2>/dev/null)"
echo "terminal size: ${SIZE:-unknown} (target 100x30)"

DISPLAY="$DISP" ffmpeg -y -f x11grab -video_size 920x660 -i "$DISP" \
  -frames:v 1 "$OUT" 2>/tmp/udu-ff.log
rc=$?
if [ $rc -ne 0 ] || [ ! -s "$OUT" ]; then
  echo "capture failed (rc=$rc): $(tail -3 /tmp/udu-ff.log)"
  exit 1
fi
echo "captured: $OUT"
