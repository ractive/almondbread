#!/bin/sh
# Runs the study at every location (CPU only, deterministic). Output: out/<loc>.txt and
# out/<loc>/*.png. Heavy views (Feigenbaum: 200k iterations, x4 extension) run smaller.
set -e
cd "$(dirname "$0")"
cargo build --release
bin=target/release/dense-sampling
for loc in dense-5e-14 minibrots-5e-12 filament-1e6 deep-5e-14 seahorse-1e5 neck-2e5 elephant minibrot-p4 root-third; do
    $bin run --loc $loc --size 800x500 --frames 12 --out out > out/$loc.txt
done
$bin run --loc feigenbaum --size 400x250 --frames 6 --out out > out/feigenbaum.txt
