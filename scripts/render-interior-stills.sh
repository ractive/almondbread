#!/bin/sh
# Headless stills of every interior colouring (kb: techniques/coloring-methods, "Interior
# colouring as built"): home view, the period-3 bulb and a deep minibrot.
# Usage: scripts/render-interior-stills.sh [out-dir] (default build/stills)
set -e
cd "$(dirname "$0")/.."
OUT="${1:-build/stills}"
BIN=target/release/almondbread
mkdir -p "$OUT"
cargo build --release -q -p almondbread-cli
for m in flat bubbles period angle distance; do
    "$BIN" render "$OUT/home-$m.png" --size 960x600 --look aurora --extend 16 --inside "$m"
    "$BIN" render "$OUT/bulb-$m.png" --size 960x600 --look aurora --extend 16 --inside "$m" \
        --center -0.1225,0.7445 --scale 0.0006
    "$BIN" render "$OUT/sea8007-$m.png" --size 960x600 --look aurora --extend 16 --inside "$m" --location sea-8007
    "$BIN" render "$OUT/needle305-$m.png" --size 960x600 --look aurora --extend 16 --inside "$m" --location needle-1e305
done
