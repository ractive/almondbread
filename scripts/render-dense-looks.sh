#!/bin/sh
# Dense-region stills of a look (the near-boundary band where the palette cycles faster than
# a pixel; kb: techniques/coloring-methods, "Dense regions"). Usage:
#   scripts/render-dense-looks.sh [out-dir] [look] [extra render flags...]
set -e
cd "$(dirname "$0")/.."
OUT="${1:-build/dense}"
LOOK="${2:-aurora}"
shift 2 2>/dev/null || shift $#
BIN=target/release/almondbread
mkdir -p "$OUT"
cargo build --release -q -p almondbread-cli
"$BIN" render "$OUT/needle-$LOOK.png" --size 850x700 --look "$LOOK" --extend 16 \
    --center -1.7686774605423412,0.00423684791873126 --scale 1.875e-6 "$@"
"$BIN" render "$OUT/deep5e14-$LOOK.png" --size 864x558 --look "$LOOK" --extend 16 --location deep-5e-14 "$@"
"$BIN" render "$OUT/seahorse-$LOOK.png" --size 864x558 --look "$LOOK" --extend 16 --location seahorse-1e5 "$@"
"$BIN" render "$OUT/filament-$LOOK.png" --size 864x558 --look "$LOOK" --extend 16 \
    --center -0.10109636384562,0.95628651080914 --scale 2e-6 "$@"
