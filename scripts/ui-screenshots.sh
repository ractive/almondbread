#!/bin/sh
# Screenshots of the control center and the overlays from background windows (`--ui-demo`):
# the window opens behind every other window and never takes focus; the app draws its own
# window into a PNG (AppKit for the SwiftUI content, Rust for the Metal canvas with overlays),
# so no screen-recording permission is needed. Usage: scripts/ui-screenshots.sh [app] [out-dir]
set -e
cd "$(dirname "$0")/.."
APP="${1:-.build/xcode/Build/Products/Release/Almondbread.app/Contents/MacOS/Almondbread}"
OUT="${2:-build/screens}"
mkdir -p "$OUT"
for scene in orbit interior deep; do
    LOG="$OUT/ui-demo-$scene.log"
    rm -f "$LOG"
    "$APP" "--ui-demo=$scene" "--ui-demo-out=$OUT" > "$LOG" 2>&1 &
    PID=$!
    n=0
    until grep -q "ui-demo:" "$LOG" 2>/dev/null; do
        sleep 1
        n=$((n + 1))
        if [ "$n" -gt 120 ]; then echo "ui-demo $scene: timed out"; kill "$PID"; exit 1; fi
    done
    grep "ui-demo" "$LOG"
    kill "$PID" 2>/dev/null || true
done
ls -1 "$OUT"/*.png
