# dense-sampling: R3 prototype

CPU study of how to compute fewer pixel-iterations in dense boundary regions for the same
perceived image (kb: `techniques/adaptive-sampling`, `synthesis/dense-region-sampling`).
Deterministic numbers only (iterations, samples, OKLab ΔE, flicker); no timing, no GPU.

```sh
cargo build --release
target/release/dense-sampling run --loc dense-5e-14 [--size 800x500] [--frames 12] [--out out]
    [--no-motion] [--sa-only]
./run-all.sh            # all study locations -> out/<loc>.txt, out/<loc>/*.png
```

## How it works

- `sample.rs`: direct f64 iteration from z = 0 with the kernel's outputs (escape n, smooth
  offset, DE in pixels, normal angle) and the derivative interior test; samples run to 4x the
  location's limit (the app's extension passes). Records when the pixel footprint
  `|dz/dc| * pixel` reaches 1e-3 ... 1.
- `shade.rs`: CPU port of the kernel's `shade` with the default look (Aurora: palette
  anti-aliasing, DE outline, lighting); OKLab ΔE x 100.
- `lattice.rs`: 5 x 5 samples per output pixel over the view plus a scripted pan of 1.2, 0.6 px
  per frame (12 frames), so every frame's samples and reference come from one lattice.
- Reference: the box mean of the 16 lattice samples per pixel that no strategy uses.
- `strategies.rs`, `sparse.rs`: 1 spp centre, FL adaptive 3 x 3 AA, colour guessing,
  Mariani-Silver interior fill, footprint cut, sparse sampling with texture fill.
- `branch.rs`, `sa.rs`: local series approximation (seed orbit with dz/dc and d²z/dc², branch
  when the quadratic term exceeds eps of the linear one over the region): AA samples off their
  pixel, pixels off a tile centre, and nested tile -> pixel -> AA.
- `metrics.rs`: ΔE statistics, flicker under the pan, a simplified FL temporal accumulation.

`out/` is ignored by git.
