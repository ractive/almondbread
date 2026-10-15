#!/bin/sh
# R4: all flights, all strategies (kb: synthesis/motion-sample-reuse). Output: $OUT/<flight>/.
# Seahorse at 1280x720, the others at 960x540, two at a time (the simulator is partly serial).
# MetalFX comparison: cargo build --release --features metalfx, then the `mfx` line below.
# Usage: ./run-all.sh [out-dir]
set -e
cd "$(dirname "$0")"
OUT=${1:-out}
BIN=./target/release/motion-reuse
F=../../scripts/flights
COMMON="--eval-every 10 --still-every 150 --budget 1.6e8 --fl-ratios 0.25,0.5 --out $OUT"
$BIN --script $F/seahorse-dive.json --size 1280x720 $COMMON
$BIN --script $F/pan-1e5.json --size 960x540 $COMMON &
$BIN --script $F/filament-cruise.json --size 960x540 $COMMON
wait
$BIN --script $F/fast-dive.json --size 960x540 $COMMON &
$BIN --script $F/deep-zoom.json --size 960x540 $COMMON
wait
# MetalFX (temporal/spatial at 1/2 and 1/2.67 per axis) vs our compose at the same ratios:
# ./target/release/motion-reuse --script $F/seahorse-dive.json --size 1280x720 --set mfx \
#   --eval-every 10 --still-every 150 --budget 1.6e8 --fl-ratios 0.5,0.375 --mfx-ratios 0.5,0.375 --out $OUT/mfx
