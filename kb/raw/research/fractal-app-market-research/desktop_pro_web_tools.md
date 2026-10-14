# Desktop, Steam, web and pro fractal tools (non-App-Store), state October 2026

Scope: competitor profile of the non-App-Store fractal ecosystem, to position almondbread (native macOS/iOS deep-zoom Mandelbrot explorer: 120 Hz flight, perturbation + LA, autopilot, one-tap zoom video, share links, educational overlay). Algorithm details for KF2+, Fraktaler 3, Imagina, FractalShark, Fractal Zoomer, Mandel Machine, Ultra Fractal, XaoS, Desarso mandelbrot-webgpu, swift-mandelbrot, Very Plotter and fractalshades are already in `kb/wiki/implementations/*.md`; this file covers the market/user angle and adds the tools not covered there.

Method notes: GitHub stars, push dates and release dates were pulled with `gh api` on 2026-10-14 (cited to the repo URL). Steam review counts come from the public `appreviews` JSON endpoint and `appdetails` on 2026-10-14 (cited to the store page). SourceForge download counts come from the SourceForge stats JSON for 2025-10-01..2026-10-13 (cited to the project page). fractalforums.org could not be fetched (redirect loop), so forum activity is a gap.

## Q1. Competitor profiles: platforms, price, activity, depth, rendering mode, GPU, video, sharing, UX, community

### Takeaway
The serious deep-zoom tools (KF2+, Fraktaler 3, Imagina, FractalShark, Mandel Machine) are free, Windows-first, offline/tile renderers with expert UX and tiny audiences (tens of GitHub stars, ~1.5k SourceForge downloads/year for Fractal Zoomer); none offers real-time flight at unlimited depth, one-click video, or a native Mac build. The only commercial cross-platform pro tool with deep zoom is Ultra Fractal ($29-$99, animation only in the $99 tier). Steam has no deep-zoom Mandelbrot explorer with traction: the fractal titles that sell are games (Fractal Block World, 1,652 reviews) or flame tools (JWildfire, 106 reviews); a Mandelbrot visualizer ("RealMandelRender") launches Nov 2026 Windows-only.

### Summary table (all cells sourced in the findings below)

| Tool | Platforms (native Mac?) | Price / licence | Last release / activity | Max depth | Real-time vs offline | GPU | Video workflow | Community signal |
|---|---|---|---|---|---|---|---|---|
| Kalles Fraktaler 2+ | Windows (Wine on Linux); no Mac | Free, AGPL-3.0+ | 2.15.5.2, 2022-06-07; "mostly stopped" | ~1e9800+ (floatexp; MPFR 2 Gbit) | Offline tiles, progressive | OpenCL (optional) | Exp-map EXR -> zoomasm, or KFB -> KF Movie Maker (After Effects) | Historic standard; no stars metric (self-hosted) |
| Fraktaler 3 | Linux, Windows, Android, Web (slow); no Mac build listed | Free, AGPL-3.0 | 3.1, 2025-12-15 | Unlimited (floatexp/doubleexp) | Interactive GUI + batch | OpenCL | Zoom-out sequence -> zoomasm | Small (self-hosted) |
| zoomasm | Linux (tested), Windows (untested build), macOS untested | Free, AGPL-3.0+ | 3.4, 2025-06-11 | n/a (assembler) | Offline | OpenGL 3.3+ | Is the video tool | Small |
| Imagina | Windows | Free, AGPL-3.0 | No releases; last push 2024-01-23; "being rewritten" | Unlimited | Interactive explorer | None documented | None documented | 30 stars |
| FractalShark | Windows (CUDA), Linux since v0.53; no Mac/AMD | Free, GPL-3.0 | 0.545, 2026-10-10 | ~1e650452 demo | Interactive, research tool | NVIDIA CUDA only | Not a focus | 28 stars |
| Fractal Zoomer | Java 1.8+: Win/Mac/Linux (JVM) | Free, GPL-3.0 | 1.0.9.5, 2026-01-09; push 2026-10-13 | Deep (perturbation+BLA) | Offline render with GUI | Not documented | Not on project page | 33 stars; ~1.5k SF downloads/yr |
| Mandel Machine | Windows 64-bit | Freeware (unverified) | Thread from 2014; abandoned (unverified) | "~8000 magnifications" (likely 1e8000-ish, interpretation) | Offline | Unknown | None | Legacy |
| Ultra Fractal 6 | Windows + macOS; Apple Silicon since 6.06 | Express $29 / Creative $49 / Extended $99 | v6.06 (date not shown) | Arbitrary precision + perturbation | Offline render | Not documented | Animation only in Extended ($99) | Long-running commercial |
| XaoS | Win/Mac/Linux (Qt) + web xaos.app | Free, GPL-2.0+ | 4.3.8, 2026-09-11 | No deep zoom (hardware float, unverified) | Real-time | No | Recording via own format (not researched) | 625 stars; 681 downloads of 4.3.8 |
| Mandelbulber 2 (3D) | Win/Linux/macOS incl. M1 DMG | Free, GPL-3.0 | 2.34, 2025-10-05; push 2026-10-11 | n/a (3D) | Offline (preview) | OpenCL | Built-in keyframe animation | 1,072 stars; ~36.5k GH release downloads total; ~10.3k SF/yr |
| Mandelbulb 3D | Windows | Free, LGPL-2.1 | v1.99, 2017-12-07; last push 2023-03-05 | n/a (3D) | Offline | No | Keyframes | 203 stars |
| Chaotica 2 (flames) | Win/Mac/Linux; Mac App Store "Lite" | Free (1.23 MP cap) / HD ~EUR25 / Studio ~EUR89 | Chaotica 2 (date not shown) | n/a | Offline | Multi-GPU (OpenCL) | Animation supported | Commercial niche |
| Apophysis / 7X (flames) | Windows; Mac only via Wine wrapper (ApoMac) | Free | 2.09 (2009); 7X legacy | n/a | Offline | No | Limited | ~3.6k + ~1.6k SF downloads/yr |
| JWildfire (flames) | Java Win/Mac/Linux; Steam Win; Mac App Store (8.50) | Free/OSS; Steam $11.99 | 9.00, 2025-10-01 | n/a | Offline + GPU via Swan | JWildfireSwan GPU backend | Built-in animation | 258 stars; Steam 106 reviews |
| Iterated Dynamics (Fractint successor) | Win/Linux/macOS arm64+x64 | Free (licence not checked) | v1.4.1, 2026-06-21 | Fractint-era arbitrary precision (not verified) | Offline | No | No | 51 stars |
| mandelbrot.site | Web (Rust/Wasm, Leaflet tiles) | Free, open source | push 2026-09-01 | "Unlimited zoom" via perturbation | Tiled, progressive | Not stated (CPU workers) | Video recording, PNG with params, share links | 330 stars |

### Cited findings

**Kalles Fraktaler 2+ (KF2+)**
- Windows binaries (64-bit, 32-bit included), cross-compiled from Linux with MINGW64, "also work in Wine on Linux"; macOS is not mentioned — [KF page](https://mathr.co.uk/kf/kf.html)
- Latest kf-2.15.5.2 dated 2022-06-07; development "mostly stopped"; known bug "Crash when zooming past 1e291 in some fractals"; AGPLv3+ — [KF page](https://mathr.co.uk/kf/kf.html)
- The maintainer's page itself points users to Imagina, fractalshades, Fractal Zoomer, FractalShark, Fraktaler 3, m-perturbator-gtk — [KF page](https://mathr.co.uk/kf/kf.html)
- Minibrot at 5e-433: ~20 min in KF (NanoMB1) vs just over 2 min in fractalshades/Fraktaler 3 vs 30 s in Fractal Zoomer with BLA + boundary tracing (Ryzen 7 2700X, 16 threads) — [KF page](https://mathr.co.uk/kf/kf.html)
- KF manual warns "glitch low tolerance" can make a file render correctly but "16x longer"; and that resizing the window during "examine zoom sequence auto solve glitches" corrupts the zoom-sequence data — [KF manual](https://mathr.co.uk/kf/manual.html)
- No in-app video export; video is made by exporting exponential-map EXR keyframes for zoomasm (KF 2.15.1+) — [zoomasm](https://mathr.co.uk/zoomasm/), [Making Fractal Zoom Videos (mathr, 2020-10-28)](https://mathr.co.uk/web/making-fractal-zoom-videos.html)

**Fraktaler 3**
- "cross-platform program (Linux, Windows, Android, Web)"; downloads: source 7z, PDF docs, Windows 7z, Android APK (Android 5+); macOS not mentioned — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)
- Web build at fraktaler.mathr.co.uk/live/latest requires SharedArrayBuffer; "Performance is significantly slower than native versions" — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)
- v3.1 dated 2025-12-15 ("28 git commits since version 3") — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)
- "The render window can be used to render zoom out sequences", assembled with zoomasm; recommended zoom-out factor 2 + Exponential Map; resume interrupted sequences from a saved key frame — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)
- Learning-curve signals from the docs: without "wisdom" benchmarking "placeholder defaults are used, which may be suboptimal"; "Some images may need the iteration limit to be millions"; advanced settings: "sometimes bad images can result"; Windows may need Mesa 3D, old hardware `LIBGL_ALWAYS_SOFTWARE=1` — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)

**zoomasm**
- Assembles zoom videos from exponential-map EXR keyframes (with raw iteration data), colours with GLSL fragments, syncs to WAV/FLAC soundtrack with a timeline, 2D and 360° projections; AGPL-3.0+ — [zoomasm](https://mathr.co.uk/zoomasm/)
- Platforms: Linux tested (Debian Buster/Bullseye/Bookworm); Windows build "not tested"; macOS "not tested" — [zoomasm](https://mathr.co.uk/zoomasm/)
- Requirements: 8 GB RAM, OpenGL 3.3+, 8 GB VRAM recommended; FFmpeg binary required; ~16 layers for 2D, 25+ for 360°; raw PPM output ~21 GB per minute at 1080p60; "rendering keyframes in KF is often the slowest step" — [zoomasm](https://mathr.co.uk/zoomasm/)
- KF workflow is manual: set window 9:1 (e.g. 1152x128), enable Exponential Map, choose EXR channels, "Store zoom-out sequence", quit, then pick keyframe dir, palette, FFmpeg binary and output in zoomasm — [zoomasm](https://mathr.co.uk/zoomasm/)
- Latest 3.4 "bona sort" 2025-06-11; known bugs: first frame sometimes wrong; GLSL `getInterior()` incompatible with Fraktaler-3; v4 (timeline sequencer for uniforms) in progress — [zoomasm news](https://mathr.co.uk/web/zoomasm.html)

**Imagina**
- Windows, C++/MPIR; README: "The project is being rewritten. This repository may be renamed and replaced"; no releases; last push 2024-01-23; 30 stars, AGPL-3.0 — [GitHub 5E-324/Imagina](https://github.com/5E-324/Imagina) (gh api 2026-10-14)

**FractalShark**
- Self-described "an experimental research project, not a polished end-user application"; NVIDIA CUDA (compute 7.5+), native Linux since v0.53, AMD "never planned" — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- Latest release 0.545 on 2026-10-10; 28 stars, 2 forks, GPL-3.0 — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark) (gh api)
- Deepest built-in example ~1e650452 — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)

**Fractal Zoomer**
- Java (>1.8), "does not require installation", Linux/Mac/Windows; features "Mandelbrot deep zooms", "Perturbation Theory"; last update 2026-01-09 (FractalZoomer.jar 41.3 MB); 11 downloads that week; 1 rating (4.0/5) — [SourceForge Fractal Zoomer](https://sourceforge.net/projects/fractalzoomer/)
- SourceForge downloads 2025-10-01..2026-10-13: 1,514 (OSes include Windows, Linux, Macintosh, Android) — [SourceForge Fractal Zoomer](https://sourceforge.net/projects/fractalzoomer/) (stats JSON)
- GitHub mirror: 33 stars, release 1.0.9.5 on 2026-01-09, last push 2026-10-13, GPL-3.0 — [GitHub hrkalona/Fractal-Zoomer](https://github.com/hrkalona/Fractal-Zoomer)
- Mac runs it via JVM only (not a native app; inference from Java-jar distribution).

**Mandel Machine**
- Fractal Forums thread started by Botond Kósa, Feb 2014: Mandelbrot explorer with high-resolution rendering (images up to ~23000x23000), deep zoom ("roughly 8000 magnifications" per search summary — likely means ~1e8000 order; unverified), PNG saving; forum marked as archived/"no longer maintainable" — [fractalforums.com Mandel Machine](https://www.fractalforums.com/mandel-machine/mandel-machine/)
- No current download, licence or version found (gap).

**Ultra Fractal 6**
- Windows and macOS; the 6.06 update "adds Apple Silicon support"; free trial with tutorials; v6 includes "fast deep zooming"; copyright through 2025 — [ultrafractal.com](https://www.ultrafractal.com/)
- Editions: Express $29/€29/£24 (single-layer, deep zooming, any-size render), Creative $49/€49/£39 (+layers, masks, transformations), Extended $99/€99/£79 (+animation and network calculations); upgrade discounts; site licences on request — [Ultra Fractal shop](http://www.ultrafractal.com/shop)
- Former Standard/Animation editions were renamed Creative/Extended; Express is new — [Ultra Fractal editions](https://ultrafractal.com/editions.html)
- So a UF user who wants a deep-zoom *video* pays $99 — [Ultra Fractal shop](http://www.ultrafractal.com/shop)

**XaoS**
- Real-time zoomer, Qt for Windows/macOS/Linux plus web version at xaos.app since 4.3 (Sept 2023) — [GitHub XaoS](https://github.com/xaos-project/XaoS)
- Release 4.3.8 on 2026-09-11; 625 stars; release assets downloaded 205 (zip) + 476 (Windows setup) times; GPL-2.0 — [GitHub XaoS](https://github.com/xaos-project/XaoS) (gh api)
- Listed by Fractal Foundation as the free open-source real-time zoomer for Mac and Windows — [Fractal Foundation software list](https://fractalfoundation.org/resources/fractal-software/)

**Mandelbulber 2 (3D)**
- Release 2.34 on 2025-10-05 with assets for Windows (setup + standalone), Linux AppImage, source, and `mandelbulber2-macOS-2.34-xcode15.4-m1.dmg` (native Apple Silicon); 1,072 stars, 138 forks, last push 2026-10-11; GPL-3.0; ~36,486 cumulative GitHub release-asset downloads — [GitHub mandelbulber2](https://github.com/buddhi1980/mandelbulber2) (gh api)
- SourceForge downloads 2025-10..2026-10: 10,327 — [SourceForge mandelbulber](https://sourceforge.net/projects/mandelbulber/) (stats JSON)

**Mandelbulb 3D**
- Last tagged release v1.99 on 2017-12-07; last push 2023-03-05; 203 stars; LGPL-2.1 — [GitHub thargor6/mb3d](https://github.com/thargor6/mb3d) (gh api)
- No source found confirming a Mac route (Wine/Whisky) — gap.

**Chaotica 2 (flame fractals)**
- Downloads for Windows, Mac OS and Linux; Mac App Store listing for "Chaotica Lite 2"; multi-GPU rendering (OpenCL); © 2013-2026 — [chaoticafractals.com](https://chaoticafractals.com/)
- Free version for noncommercial use capped at 1.23 MP; HD licence ~4 MP and 720p animation; Studio no limits; Black Friday prices HD €20 (usually €25), Studio €71 (usually €89) (year of sale not shown) — [chaoticafractals.com](https://chaoticafractals.com/); [Chaotica node/4](https://chaoticafractals.com/node/4)

**Apophysis**
- Stable 2.09 (2009-09-10), Windows; Mac users relied on ApoMac (Wine launcher) or Apophysis-j (Java port, criticised in 2013 for small images and limited performance) — [Wikipedia Apophysis](https://en.wikipedia.org/wiki/Apophysis_(software)); [Softpedia ApoMac](https://mac.softpedia.com/get/Graphics/ApoMac.shtml)
- SourceForge downloads 2025-10..2026-10: Apophysis 3,571; Apophysis 7X 1,617 — [SourceForge apophysis](https://sourceforge.net/projects/apophysis/), [SourceForge apophysis7x](https://sourceforge.net/projects/apophysis7x/) (stats JSON)

**JWildfire / JWildfireSwan (flames)**
- JWildfire 9.00 released 2025-10-01; Win/Mac/Linux zip (needs Java), Windows via Steam, macOS via App Store (8.50, Ventura-compatible); "free and open-source"; JWildfireSwan is the GPU backend, Steam Early Access 2025-03-14, 1.0 on 2025-09-19 — [JWildfire site](https://jwildfire.overwhale.com/)
- Steam: JWildfire $11.99, Windows only, 106 reviews "Very Positive" (103 positive); JWildfireSwan $24.99, Windows+Linux, 20 reviews "Positive" — [Steam JWildfire](https://store.steampowered.com/app/1445310/), [Steam JWildfireSwan](https://store.steampowered.com/app/3529700/)
- GitHub: 258 stars, last push 2026-08-30 — [GitHub JWildfire](https://github.com/thargor6/JWildfire)

**Fractint legacy -> Iterated Dynamics**
- Iterated Dynamics (Fractint descendant by Richard Thomson / LegalizeAdulthood): v1.4.1 on 2026-06-21 with `mac-arm64` and `mac-x64` builds (first Mac builds appear at 1.4.0, 2026-06-19; 1.3.2 was Windows-only); 51 stars — [GitHub iterated-dynamics](https://github.com/LegalizeAdulthood/iterated-dynamics) (gh api)
- Winfract: no current activity found (gap).

**Steam fractal titles (search "fractal": 209 results; "mandelbrot": 13; "mandelbulb": 3, on 2026-10-14)** — [Steam search](https://store.steampowered.com/search/?term=mandelbrot)
- Fractal Block World (exploration game, infinite shrink into a fractal), $6.99, Windows only, 1,652 reviews "Overwhelmingly Positive" — [Steam](https://store.steampowered.com/app/1540570/)
- Fractal Fly (fly through 3D fractal landscapes), free, Win+Mac, 151 reviews "Mostly Positive" — [Steam](https://store.steampowered.com/app/1860120/)
- Fractal Alchemist (3D fractal worlds, San Base), $19.99, Windows, 29 reviews "Positive" — [Steam](https://store.steampowered.com/app/1318030/)
- MandelCraft (San Base, free, released 2026-02-27), Windows, 20 reviews "Positive" — [Steam](https://store.steampowered.com/app/4081860/)
- Fractaltopia: Edit & Visualize (music visualiser + fractal toolkit), free, Windows, 15 reviews "Mixed" — [Steam](https://store.steampowered.com/app/1507700/)
- Fractal Glide, $3.99, Windows, 12 reviews; Fractal Gallery VR, $1.99, 9 reviews — [Steam Fractal Glide](https://store.steampowered.com/app/2565200/), [Steam Fractal Gallery VR](https://store.steampowered.com/app/1081600/)
- Fractal To Desktop (live Mandelbrot wallpaper, auto-picks interesting points), $4.99, Windows, 6 reviews, 2018 — [Steam](https://store.steampowered.com/app/942770/)
- CHAOS GAME (chaos game, Mandelbrot, Julia), free, Windows, 5 reviews — [Steam](https://store.steampowered.com/app/1569980/)
- AI-VJ Live Fractal Visuals DLC ("30+ points of interest on the Mandelbrot figure"), $4.99, 0 reviews — [Steam](https://store.steampowered.com/app/2941700/)
- RealBuddhaRender (WebGPU Buddhabrot 4D viewer, ukaapple), $2.49, Windows, 1 review, released 2026-06-10 — [Steam](https://store.steampowered.com/app/4560330/)
- **RealMandelRender – Fractal Visualizer** (ukaapple): "Zoom freely, explore 132 curated destinations... 18 color palettes. Save your discoveries as images or videos"; Windows only; release date Nov 5, 2026 (upcoming) — [Steam](https://store.steampowered.com/app/5227800/)
- **Fractaria** (Tears in Rain): "Fractal art, made easy. A desktop studio for artists: explore dozens of fractal families, color them by hand, breed variants, animate them... publish to the Steam Workshop"; Windows + Mac; Q4 2026 — [Steam](https://store.steampowered.com/app/5157400/)
- No KF/Fraktaler/Ultra Fractal/Mandelbulber/Chaotica listing appears on Steam (absent from search results above) — [Steam search](https://store.steampowered.com/search/?term=fractal)

**Web explorers**
- mandelbrot.site (Ross Hill): Rust→WebAssembly in Web Workers, Leaflet map-style tiles, perturbation with one high-precision reference, "Unlimited zoom", shareable links reopening the same location/settings, high-res PNG with embedded parameters, raw iteration data download, zoom animations recorded as video — [How mandelbrot.site was built](https://mandelbrot.site/how-mandelbrot-site-was-built); 330 stars, last push 2026-09-01 — [GitHub rosslh/Mandelbrot.site](https://github.com/rosslh/mandelbrot.site)
- Desarso mandelbrot-webgpu (mandelbrot.gabrielmalek.com): GPU reference orbit, 2nd-order skip tables, 2 stars, push 2026-10-01 — [GitHub Desarso/mandelbrot-webgpu](https://github.com/Desarso/mandelbrot-webgpu)
- Fraktaler 3 web build (slower than native) — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)
- XaoS web build at xaos.app — [GitHub XaoS](https://github.com/xaos-project/XaoS)
- Very Plotter (BLA in browser, dormant since 2023-05) — [GitHub philthompson/visualize-primes](https://github.com/philthompson/visualize-primes)
- Others in search results: Mandelbrot Maps (React+WebGL), mandelbrot.silversky.dev, mandel.gart.nz — [Mandelbrot Maps](https://jmaio.github.io/mandelbrot-maps/), [Mandelbrot Viewer](https://mandelbrot.silversky.dev/), [mandel.gart.nz](https://mandel.gart.nz/)

**2024-2026 newcomers (GitHub, all tiny)**
- DeepGPU-Zoomer (WebGPU, created 2026-09-19, 11 stars) — [GitHub](https://github.com/byronbuzz/DeepGPU-Zoomer)
- gcollombet/mandelbrot ("realtime... deep zoom, using rust wasm and wgpu", 2025-09, 3 stars) — [GitHub](https://github.com/gcollombet/mandelbrot)
- fractious (Wasm + WebGPU deep zoom, push 2026-10-13, 1 star) — [GitHub](https://github.com/rowan-m/fractious)
- mandel_zoom_abyss (WebGPU + Wasm GMP perturbation, real-time ~1e15, 2026-05, 5 stars) — [GitHub](https://github.com/voich2014/mandel_zoom_abyss)
- fractadyne (native Windows Rust wgpu+egui, ultra-deep perturbation, 2026-06) — [GitHub](https://github.com/WindySnowOwl/fractadyne)
- mandelbloom (Windows CUDA perturbation+BLA, D3D12 HDR, 2026-09) — [GitHub](https://github.com/no-more-secrets/mandelbloom)
- flutter-fractal-forge (Flutter, GPU shaders, deep zoom, 1,500+ fractal catalogue, 2026-02, 4 stars) — [GitHub](https://github.com/TrebuchetDynamics/flutter-fractal-forge)
- AppleCore ("Deeply zoom into the Mandelbrot set and create videos from your journey", 2024-11, 1 star) — [GitHub](https://github.com/607011/AppleCore)
- DeepDrill (Dirk Hoffmann; perturbation + SA; "lean" teaching codebase; GPL-3; v3.2; inspired by Maths Town; 15 stars; push 2025-09) — [DeepDrill site](https://dirkwhoffmann.github.io/DeepDrill/), [GitHub](https://github.com/dirkwhoffmann/DeepDrill)
- rust-fractal-core (perturbation + SA, 42 stars, push 2026-08) — [GitHub](https://github.com/rust-fractal/rust-fractal-core)
- swift-mandelbrot (SwiftUI+Metal, iPhone/iPad/Mac, >1e1000, zoom movies, `mandelbrot://` links; 0 stars, no licence) — [GitHub drquicksilver/swift-mandelbrot](https://github.com/drquicksilver/swift-mandelbrot)

### Inferences
- The deep-zoom "pro" segment is a free, AGPL/GPL, single-maintainer ecosystem. Nobody sells deep-zoom Mandelbrot software except Ultra Fractal, so almondbread competes against "free but hard" rather than against a polished paid incumbent.
- Community size is small and measurable: deep-zoom renderers have 28-42 GitHub stars each and Fractal Zoomer ~1.5k SF downloads/year; the 3D (Mandelbulber, ~10k SF/yr + 1k stars) and flame (JWildfire, Apophysis) communities are larger than the 2D deep-zoom one.
- On Steam, "explore an infinite fractal" sells as a *game* (Fractal Block World 1,652 reviews) far better than any tool; utility-style fractal apps sit at 0-30 reviews. Steam is a weak channel for a Mandelbrot explorer unless framed as an experience.
- Real-time flight at unlimited depth is not offered by any established tool: KF/F3/FZ are tile renderers, XaoS is real-time but shallow, web explorers are tiled/progressive. This is almondbread's clearest differentiator; swift-mandelbrot and a handful of 2025-26 WebGPU hobby projects are the only ones attempting it.
- mandelbrot.site already bundles share links + PNG with params + video recording in a free browser tool; "shareable links" and "video export" are not unique by themselves — the combination with depth + 120 Hz + native is.
- Two Steam newcomers (RealMandelRender: curated destinations + video; Fractaria: Mac+Win studio) in Q4 2026 indicate others see the same "easy fractal" opening.

### Gaps
- fractalforums.org activity (members, posts/day) not retrieved: site returned a redirect loop.
- Max zoom depth for Mandel Machine ("8000 magnifications" phrasing) unverified; licence/download not found.
- Ultra Fractal 6.06 release date and upgrade prices not shown on fetched pages.
- Chaotica current (non-sale) prices and Apple Silicon status not confirmed; Black Friday year unknown.
- Mandelbulb 3D on macOS (Wine/Whisky/CrossOver) not confirmed by any source.
- KF, Fraktaler 3 and zoomasm are self-hosted (code.mathr.co.uk), so no download or star counts exist.
- Fractal Zoomer video/sequence export capability not confirmed (SourceForge page silent).
- XaoS max depth not verified from a primary source.
- Steam search API only exposes title matches; tools with no "fractal"/"Mandelbrot" in name/tags may be missed.

## Q2. How YouTube deep-zoom creators produce videos today, render times, pain points

### Takeaway
The dominant pipeline is still Kalles Fraktaler 2+ (Windows, unmaintained since 2022) exporting keyframes, assembled either with zoomasm (free, Linux-tested) or Maths Town's KF Movie Maker After Effects plug-in (needs paid Adobe AE); Fraktaler 3 slots into the same zoomasm path. Renders for record-depth videos take weeks to months of CPU time, and every step (keyframe export, assembly, colouring, encoding) is manual.

### Cited findings
- mathr's 2020 tutorial: render with KF 2.15 (hybrids, exponential map, OpenCL), assemble with zoomasm 1.0 (GLSL colouring, timeline to sync to music); the exponential map is "much more efficient than frames at 2x zoom levels" — [Making Fractal Zoom Videos](https://mathr.co.uk/web/making-fractal-zoom-videos.html)
- Fraktaler 3 renders zoom-out sequences for zoomasm (zoom factor 2, exponential map), with resume from a saved key frame — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)
- Maths Town (major YouTube deep-zoom channel) publishes **KF Movie Maker**, a free AGPL After Effects plug-in that turns KF `.kfb` iteration files + one `.kfr` colour file into a continuous zoom; "Adobe After Effects is not free, you will need to buy it, or setup a subscription" — [KFMovieMaker User Guide](https://www.maths.town/after-effects-plugins/kfmoviemaker/kfmoviemaker-user-guide/) (inference: Maths Town's own pipeline is KF + AE; the videos page itself does not name software — [Maths Town videos](https://www.maths.town/videos/))
- KF Movie Maker constraints: zoom size must be exactly 2 ("no other value will work"); images saved in zoom-out order; composition must match export size (plug-in "does not scale"); keyframes animated manually (~1.5 s per keyframe as a starting point); render via Adobe Media Encoder — [KFMovieMaker User Guide](https://www.maths.town/after-effects-plugins/kfmoviemaker/kfmoviemaker-user-guide/)
- KF Movie Maker pain points: KF export can take "hours, days, weeks or months"; large disk use; 4K/8K "can get very slow"; animating colour invalidates caches; layer sampling "is crashing with this enabled... save your work often!"; frame-by-frame colouring flickers, cached frames show seams; fractal zooms need higher bitrate than YouTube gives, so slower zooms may be needed — [KFMovieMaker User Guide](https://www.maths.town/after-effects-plugins/kfmoviemaker/kfmoviemaker-user-guide/)
- Maths Town render times from video descriptions: "The Hardest Zoom" (2020) took over a month on a 64-core CPU, late frames over 2 days each, depth ~7e1299 — [archive.org](https://archive.org/details/youtube-_zGNMiOBaPA); "The Perpendicular Mandelbrot" (2019) ~4 months of data rendering plus ~55 h After Effects export/8K encode — [archive.org](https://archive.org/details/youtube-HVJXdZgWnF8); "The Edge of Infinity" (e2011) ~2 weeks — [archive.org](https://archive.org/details/youtube-u1pwtSBTnPU) (figures via search summary of archived descriptions; not opened individually)
- Maths Town depths: "A Trip to Infinity" 1.2e1077 4K60 ("An enormous amount of CPU time"), "Deepish" 4e2656 — [Maths Town videos](https://www.maths.town/videos/)
- zoomasm: keyframe rendering in KF "is often the slowest step"; raw PPM ~21 GB/min at 1080p60; needs FFmpeg; Windows build untested; macOS untested — [zoomasm](https://mathr.co.uk/zoomasm/)
- KF glitch handling costs time: "glitch low tolerance" can be 16x slower; zoom-sequence data corruption if the window is resized during auto glitch solving — [KF manual](https://mathr.co.uk/kf/manual.html)
- DeepDrill's author cites a Maths Town video as inspiration and says images "that once took weeks or months can now be computed in minutes" — [DeepDrill](https://dirkwhoffmann.github.io/DeepDrill/)

### Inferences
- Pain points almondbread directly addresses: Windows-only tooling (KF, KF Movie Maker practically, Imagina, FractalShark), multi-tool handoffs (KF -> EXR/KFB -> zoomasm/AE -> FFmpeg/Media Encoder), hand-set keyframes and timing, glitch tuning, weeks of CPU time. A one-tap export that renders keyframes on the GPU with BLA/LA and assembles the movie in-app removes every handoff.
- Modern BLA renderers already cut render time ~10x vs KF's SA (20 min -> 2 min -> 30 s at 5e-433), so the "weeks" figures are partly legacy (2018-2020 KF+SA); almondbread's marketing should benchmark against current tools, not Maths Town's 2020 numbers.
- Creators still need fine control (colour timelines, music sync, 8K, high bitrate); a "one-tap" export should have a pro escape hatch (EXR/raw data export compatible with zoomasm) to win creators.

### Gaps
- Toolchain of Seryozha and other channels not found (searches returned nothing).
- No Reddit r/fractals or fractalforums threads with recent (2025-26) render-time or pain-point reports were retrievable (Reddit mirror refused connection; fractalforums.org redirect loop).
- No direct confirmation of which tool Maths Town uses today (2025-26); KF + AE is inferred from its own plug-in.
- Whether KF Movie Maker runs on After Effects for macOS not stated.

## Q3. What is missing on macOS specifically

### Takeaway
On macOS there is no native, maintained deep-zoom Mandelbrot renderer for end users: KF2+, Fraktaler 3, Imagina, FractalShark and Mandel Machine have no Mac build; zoomasm is untested on Mac; Fractal Zoomer runs only as a Java jar. Mac users get either shallow real-time (XaoS), the paid generalist Ultra Fractal ($29-$99, Apple Silicon since 6.06), Java tools, browser explorers, or an unlicensed hobby project (swift-mandelbrot).

### Cited findings
- KF2+: Windows binaries, Wine on Linux; macOS not mentioned — [KF page](https://mathr.co.uk/kf/kf.html)
- Fraktaler 3: Linux, Windows, Android, Web; macOS not mentioned; downloads list no Mac binary — [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)
- zoomasm: macOS "not tested" — [zoomasm](https://mathr.co.uk/zoomasm/)
- Imagina: Windows — [GitHub Imagina](https://github.com/5E-324/Imagina)
- FractalShark: CUDA/NVIDIA only (Windows, Linux); impossible on Apple Silicon — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- Mandel Machine: Windows 64-bit — [fractalforums.com](https://www.fractalforums.com/mandel-machine/mandel-machine/)
- Mandelbulb 3D: Windows (Delphi; last release 2017) — [GitHub mb3d](https://github.com/thargor6/mb3d)
- Apophysis: Windows; Mac via Wine wrapper ApoMac or weak Java port — [Wikipedia](https://en.wikipedia.org/wiki/Apophysis_(software)), [Softpedia ApoMac](https://mac.softpedia.com/get/Graphics/ApoMac.shtml)
- Steam fractal tools are nearly all Windows-only: JWildfire, Fractal Alchemist, MandelCraft, Fractal To Desktop, RealMandelRender, Fractal Block World; Mac exceptions: Fractal Fly (free) and Fractaria (Q4 2026) — Steam pages cited in Q1
- Available on Mac: Ultra Fractal 6 (Apple Silicon since 6.06) — [ultrafractal.com](https://www.ultrafractal.com/); XaoS (real-time, shallow) — [GitHub XaoS](https://github.com/xaos-project/XaoS); Mandelbulber 2.34 M1 DMG (3D) — [GitHub mandelbulber2](https://github.com/buddhi1980/mandelbulber2); Chaotica (flames) — [chaoticafractals.com](https://chaoticafractals.com/); JWildfire (Java + Mac App Store 8.50) — [JWildfire](https://jwildfire.overwhale.com/); Iterated Dynamics arm64 since June 2026 — [GitHub iterated-dynamics](https://github.com/LegalizeAdulthood/iterated-dynamics); Fractal Zoomer and fractalshades via Java/Python — [SourceForge FZ](https://sourceforge.net/projects/fractalzoomer/), [GitHub Fractalshades](https://github.com/GBillotey/Fractalshades)
- Mac-oriented 2D apps found by search are art/image tools, not deep zoomers: Mandelbrot Image Generator ($9.99, 4000x4000 images), MandArt (free, art/printing); RetinaMandelbrot and Mandelbrot Metal (zoom to 1e16) are iOS — [App Store Mandelbrot Image Generator](https://apps.apple.com/us/app/id438650163), [App Store MandArt](https://apps.apple.com/us/app/mandart/id6445924588), [App Store Mandelbrot Metal](https://apps.apple.com/hn/app/mandelbrot-metal/id6749807290) (App Store detail is another researcher's scope)
- swift-mandelbrot: SwiftUI + Metal for iPhone/iPad/Mac, >1e1000, zoom movies, `mandelbrot://` bookmarks; no licence, 0 stars, needs Xcode 26.2 build — [GitHub swift-mandelbrot](https://github.com/drquicksilver/swift-mandelbrot)
- KF's OpenCL GPU path and Fraktaler 3's OpenCL rely on an API Apple deprecated (inference; Apple deprecation not sourced here); FractalShark relies on CUDA — [KF page](https://mathr.co.uk/kf/kf.html), [Fraktaler 3 site](https://fraktaler.mathr.co.uk/), [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)

### Inferences
- The macOS gap is concrete: an Apple Silicon Mac owner who wants the KF/zoomasm-quality deep-zoom video workflow today must use Wine/CrossOver, a Linux/Windows box, a Java jar, or a slower web build. A native Metal app with built-in video export fills an empty slot.
- GPU-wise, every serious deep-zoom GPU path (OpenCL, CUDA) is unavailable or deprecated on Mac; Metal-native float32 + extended-exponent perturbation (as swift-mandelbrot shows is feasible) is uncontested among maintained, licensed tools.
- Planned Windows/Linux ports would enter the crowded free-tool home turf (KF, F3, FZ, FractalShark, fractadyne, mandelbloom); the differentiator there must be UX (real-time flight, autopilot, one-tap video, links), not depth or speed.

### Gaps
- Whether KF/Fraktaler 3 run acceptably on Apple Silicon via CrossOver/Wine — no source found.
- DeepDrill's platform (Dirk Hoffmann is a macOS developer) not confirmed — site does not say.
- No survey of how many fractal hobbyists use Macs.
