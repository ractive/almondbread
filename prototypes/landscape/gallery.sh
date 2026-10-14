#!/usr/bin/env bash
# Renders the L0 gallery: full-size frames into out/, curated JPEGs into gallery/.
# Timings of every render go to out/timings.txt. Usage: ./gallery.sh [--quick] [--video]
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release -q
B=target/release/landscape
SIZE=1920x1080
SS=2
if [[ "${1:-}" == "--quick" ]]; then SIZE=960x540; SS=1; fi
O=out
G=gallery
mkdir -p $O $G
T=$O/timings.txt
: > $T
r() { "$B" render --size $SIZE --ss $SS "$@" | tee -a $T; }

# 1-2: height variants side by side (same shot, water only in the interior)
for L in "seahorse:--loc seahorse --scale 1e-4" "minibrot:--loc seahorse --scale 1e-6 --snap minibrot"; do
  n=${L%%:*}; a=${L#*:}
  r $a --height linear --k 0.2 -o $O/var-$n-linear.png
  r $a --height log -o $O/var-$n-log.png
  r $a --height sqrt --k 1 -o $O/var-$n-sqrt.png
  r $a --height potential --k 1 -o $O/var-$n-potential.png
done
"$B" montage --cols 2 --cell 960 --quality 80 --labels "linear k=0.2|log s=0.1 k=4 (recommended)|sqrt s=0.1 k=1|potential k=1" \
  --title "height variants: seahorse valley, S=1e-4" -o $G/01-variants-seahorse-1e-4.jpg \
  $O/var-seahorse-{linear,log,sqrt,potential}.png
"$B" montage --cols 2 --cell 960 --quality 75 --labels "linear k=0.2|log s=0.1 k=4 (recommended)|sqrt s=0.1 k=1|potential k=1" \
  --title "height variants: minibrot shore (period 39), S=1e-6" -o $G/02-variants-minibrot-1e-6.jpg \
  $O/var-minibrot-{linear,log,sqrt,potential}.png

# 3: far field: flat world vs planet curvature, linear vs log
A="--loc seahorse --scale 1e-4 --pitch -8"
r $A --height linear --k 1 --curvature 0 -o $O/far-linear-flat.png
r $A --height linear --k 1 -o $O/far-linear-curved.png
r $A --height log --curvature 0 -o $O/far-log-flat.png
r $A --height log -o $O/far-log-curved.png
"$B" montage --cols 2 --cell 960 --quality 80 --labels "linear k=1, flat world|linear k=1, curvature 150 S|log, flat world|log, curvature 150 S" \
  --title "far field: walls at the horizon" -o $G/03-far-field.jpg $O/far-{linear-flat,linear-curved,log-flat,log-curved}.png

# 4: self-similarity, same location and shot at four depths
for s in 1e-3 1e-6 1e-9 1e-12; do r --loc seahorse --scale $s -o $O/ss-seahorse-$s.png; done
"$B" montage --cols 2 --cell 960 --quality 80 --labels "S=1e-3|S=1e-6|S=1e-9|S=1e-12" \
  --title "same shot at four depths: seahorse valley, log" -o $G/04-selfsim-seahorse.jpg $O/ss-seahorse-{1e-3,1e-6,1e-9,1e-12}.png

# 5: self-similarity across locations
r --loc home --scale 1e-1 -o $O/ss-home-1e-1.png
r --loc neck --scale 1e-4 -o $O/ss-neck-1e-4.png
r --loc seahorse --scale 1e-8 --snap minibrot -o $O/ss-minibrot-1e-8.png
r --loc elephant --scale 1e-12 -o $O/ss-elephant-1e-12.png
"$B" montage --cols 2 --cell 960 --quality 80 --labels "main cardioid shore S=1e-1|-3/4 neck S=1e-4|minibrot p936 S=1e-8|elephant valley S=1e-12" \
  --title "same shot, four places and depths" -o $G/05-selfsim-locations.jpg $O/ss-{home-1e-1,neck-1e-4,minibrot-1e-8,elephant-1e-12}.png

# 6: dive: 8 frames on a glide path into a seahorse-valley boundary point
"$B" dive --loc seahorse --scale 1e-3 --scale-end 1e-10 --frames 8 --size $SIZE --ss $SS --out-dir $O/dive | tee -a $T
"$B" montage --cols 4 --cell 480 --quality 82 --labels "S=1e-3|1e-4|1e-5|1e-6|1e-7|1e-8|1e-9|1e-10" \
  --title "dive: glide path into one boundary point" -o $G/06-dive.jpg $O/dive/dive-00{0..7}.png

# 7-9: hero shots (flooded valleys: water at 0.02 S)
r --loc seahorse --scale 1e-4 --water 0.02 -o $O/hero-seahorse.png
r --loc seahorse --scale 1e-6 --snap minibrot --water 0.01 --turn-heading 10 -o $O/hero-minibrot.png
r --loc elephant --scale 1e-6 --water 0.01 --sun-az 130 --sun-el 16 -o $O/hero-elephant.png
for h in seahorse minibrot elephant; do
  "$B" montage --cols 1 --cell 1600 --quality 80 -o $G/0$([[ $h == seahorse ]] && echo 7 || ([[ $h == minibrot ]] && echo 8 || echo 9))-hero-$h.jpg $O/hero-$h.png
done

if [[ "${*}" == *--video* ]]; then
  "$B" dive --loc seahorse --scale 1e-3 --scale-end 1e-11 --frames 240 --size 640x360 --out-dir $O/dive-video | tail -1
  ffmpeg -y -loglevel error -framerate 30 -i $O/dive-video/dive-%03d.png -c:v libx264 -pix_fmt yuv420p -crf 32 -preset slow $G/10-dive.mp4
  ls -l $G/10-dive.mp4
fi
ls -l $G
