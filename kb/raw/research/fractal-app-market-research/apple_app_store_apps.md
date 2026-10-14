# Fractal explorer apps on Apple's App Store (iOS/iPadOS and Mac): competitor profile, October 2026

Method note: Store metadata (price, rating, rating count, version, release dates, size, min OS, genre, description) was pulled on 2026-10-14 from Apple's public iTunes Search/Lookup API (`https://itunes.apple.com/search` and `https://itunes.apple.com/lookup?id=<id>&country=<cc>`) for the US, GB, DE, CH, JP, AU and CA storefronts. Review snippets come from Apple's public customer-review RSS feed (`https://itunes.apple.com/<cc>/rss/customerreviews/page=1/id=<id>/sortby=mostrecent/json`), which returns only the most recent ~50 reviews that have written text. IAP prices and version histories come from WebFetch of the apps.apple.com pages. In the citations below, "Lookup" means `https://itunes.apple.com/lookup?id=<id>&country=us` and "Reviews <cc>" means the RSS feed for that storefront.

Caveat on Mac App Store data: the iTunes API returns rating 0 / count 0 for every `mac-software` item queried (Fractal Architect 5, JWildfire, MandArt, etc.), so Mac-only rating counts could not be determined. They are listed as unknown, not as zero.

## Which fractal apps exist on iOS/iPadOS and the Mac App Store (developer, platform, price/model, rating, update recency)?

### Takeaway
The Apple fractal category is long-tail and small. No fractal explorer has more than about 620 US ratings. Frax, the best-known name, has only 232 US ratings and is depth-capped at about 2e14. The 2025–2026 newcomers are what matter for positioning. At least seven apps launched or rewritten between late 2025 and October 2026 now advertise perturbation or arbitrary-precision deep zoom, and several bundle autopilot "dive", share links and video export: Mandelbrot Metal 3.0, Fractal Dive, Fractal Explorer: Deep Zoom, Ultra Mandelbrot, MandelKit, xFractal 2.0 and Retinamandelbrot. Almondbread's planned feature list (deep zoom + autopilot + video + links) is therefore not unique on paper. All of these apps have between 0 and 28 US ratings, so none has captured the market. Two apps from the requested list, Mandelbrot Maps and Fractile, were not found on the App Store.

### Cited Findings

**Established / legacy apps (most ratings)**

- **Fraksl** (Emiel Harmsen). iPhone/iPad (runs on Mac as an iPad app). Free with à-la-carte IAP plus a "full bundle". US 4.78★ / 620 ratings, the largest rating count of any fractal app found. GB 4.85/73, DE 4.79/39, CA 4.66/29, AU 4.45/31. Version 3.0.3 shipped 2026-06-15 and was released as a "complete rebuild" in 2026. First released 2017-05-26. Category: Graphics & Design. It is a video-feedback fractal visualizer (layers, camera/NDI input, MIDI/audio/gamepad modulators, video recording), not a Mandelbrot deep-zoom explorer. — [Lookup 1238167366](https://itunes.apple.com/lookup?id=1238167366&country=us); [App Store](https://apps.apple.com/us/app/id1238167366)
- **Frax** (Iter9, LLC; Ben Weiss, Kai Krause, Tom Beddard). iPhone, iPad, Apple Vision (visionOS 1+), and Mac on Apple silicon via the iPad app ("Not verified for macOS"). Free + IAP: Frax Pro Upgrade $4.99; Cloud Credits packs: 20 for $0.99, 80 for $2.99, 240 for $5.99, 1000 for $9.99. US 4.50★ / 232 ratings. GB 4.64/59, DE 4.42/86, CA 4.28/29, JP 4.43/14, AU 4.62/13, CH 4.8/10. Version 2.1.0 shipped 2026-07-11 with notes reading only "Bug fixes and improvements". The previous release was 2.0.8 on 2024-11-03, a roughly 20-month gap. Before that: 2.0.7 2024-06-30, 2.0.6 2023-06-05, 2.0 2020-05-10. First release 2013-10-02. Size 76.9 MB. Privacy label "Data Not Collected". Genres: Graphics & Design, Entertainment. — [Lookup 568827824](https://itunes.apple.com/lookup?id=568827824&country=us); [App Store page](https://apps.apple.com/us/app/frax/id568827824)
- **Mandel Draw** (Kouji Mori). iPhone/iPad. Free + one unlock IAP (price not captured). US 4.48★ / 208 ratings. GB 4.67/24, JP 4.25/20, CA 4.64/25. Version 2.1.10 shipped 2025-12-28. First release 2021-01-07. Category: Education. — [Lookup 1547068084](https://itunes.apple.com/lookup?id=1547068084&country=us)
- **Mysteries of Fractal** (弓 张 / Gong Zhang). iPhone/iPad/Mac/Watch. Paid $4.99. US 4.73★ / 164 ratings. GB 4.5/18, CA 4.63/19. Version 2.3.7 shipped 2024-01-16, but Apple pushed a recompile for watchOS 27 with no new features. Category: Education. — [Lookup 1086527481](https://itunes.apple.com/lookup?id=1086527481&country=us)
- **TeraFractal** (Math Easel, LLC). iPhone/iPad. Free. US 4.66★ / 110. Last update v1.6 on 2021-11-05. It draws IFS fractals and explicitly does not draw the Mandelbrot or Julia sets. Category: Lifestyle. — [Lookup 451021057](https://itunes.apple.com/lookup?id=451021057&country=us)
- **Fractals** (Pomegranate Apps LLC). Paid $2.99. US 4.34★ / 76. Last update 2020-05-15. — [Lookup 300542371](https://itunes.apple.com/lookup?id=300542371&country=us)
- **FractRoam** (Kaolin Fire). iPhone/iPad. Paid $0.99. US 4.29★ / 75. Version 2.1 shipped 2026-08-11 after a long gap; the previous review-visible version was 1.0.2 (2018–2020). It has a "Roam autopilot", share link and macOS screensaver. Category: Entertainment. — [Lookup 1043441882](https://itunes.apple.com/lookup?id=1043441882&country=us); [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1043441882/sortby=mostrecent/json)
- **Magic Fractals & Shapes 3D** (Marvin Krueger). Paid $1.99. US 4.82★ / 73. Last update 2021-05-25. Category: Games. There is also a sibling, **Magic Fractals 2D: Art Therapy** (free, 4.91★ / 34, updated 2025-02-13). — [Search results, fractal/mandelbulb queries](https://itunes.apple.com/search?term=fractal&country=us&entity=software&limit=50)
- **Retinamandelbrot** (Fritz Hoermann). iPhone/iPad. Free, no IAP mentioned. US 4.33★ / 66. GB 4.71/14, DE 4.8/10. It is still on v1.0, released 2020-12-29 and never updated; a GB reviewer (2026-07-12) noted that "it has never been updated". Category: Education. — [Lookup 1509664268](https://itunes.apple.com/lookup?id=1509664268&country=us); [Reviews GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=1509664268/sortby=mostrecent/json)
- **Fast Fractal** (Tom Kerrigan). Free. US 3.86★ / 36. Last update 2019-12-22. — [Search results](https://itunes.apple.com/search?term=mandelbrot&country=us&entity=software&limit=50)
- **Mandelbulb Raytracer** (Angisoft). Paid $0.99. US 2.64★ / 11. Last update 2019-12-09. It was originally written for iPhone 3GS with OpenGL ES 2.0. — [Lookup 347888739](https://itunes.apple.com/lookup?id=347888739&country=us)
- **Fractally** (Craig McPheeters). iPhone/iPad/Mac. Free (donations). US 4.12★ / 17. Last update 2022-11-17. The listing says it zooms "over 1,000,000,000,000 x". — [Search results](https://itunes.apple.com/search?term=fractal&country=us&entity=software&limit=50); [App Store FR](https://apps.apple.com/fr/app/fractally/id1534380451)
- **Jude – Fractal Explorer** (Blake Shaw). Free. 4.75★ / 12. Last update 2023-12-22. **FractalScapes** (MOEDAE) is free, 4.0★ / 12, last updated 2021. **Fractal 3D** (Nuclear Nova) is $1.99 on iOS (4 ratings, last update 2015) and $4.99 on Mac (updated 2024-10-10); the same developer sells **Video Fractal** for Mac at $11.99 (updated 2023-11-09). — [Search results](https://itunes.apple.com/search?term=fractal&country=us&entity=software&limit=50); [Mac search](https://itunes.apple.com/search?term=fractal&country=us&entity=macSoftware&limit=50)
- **Mandelbrot Trot** (Misha Software Solutions). Paid $0.99. US 3.0★ / 5. Version 26.9.0 shipped 2026-09-19. — [Lookup 1479289927](https://itunes.apple.com/lookup?id=1479289927&country=us)

**Flame/IFS creator apps (adjacent; not Mandelbrot explorers)**

- **Fractal Architect** (Centcom), iOS/iPadOS: paid $9.99. US 4.11★ / 9. Last update v1.3.4 on 2023-04-07, a "hot fix for inability to create and save new fractals". It renders flame fractals with Metal, offers 374 variations and Lua scripts, and syncs files through iCloud/Files. — [Lookup 1445109024](https://itunes.apple.com/lookup?id=1445109024&country=us)
- **Fractal Architect 5** (Mac): $19.99 with IAP for GPU rendering (OpenCL/Metal, eGPU) and an Animation Sequencer. **Fractal Architect X** (Mac): free base app with an image cap of 1024×768, plus Silver/Gold subscriptions (monthly or yearly). Both were last updated as v5.8.0 on 2023-11-28. — [Lookup 1331123802](https://itunes.apple.com/lookup?id=1331123802&country=us); [Lookup 1331145352](https://itunes.apple.com/lookup?id=1331145352&country=us)
- Other Mac-only flame and fractal tools: **JWildfireSwan** (Andreas Maschke) is $29.99, v4.13 updated 2026-09-27, first released 2025-10-06. **JWildfire** is $8.99, updated 2023-12-25. **Chaotica Lite 2** is free and was last updated 2020. **MandArt** (Bruce Johnson) is a free Mandelbrot print-art tool, v8.2 updated 2024-04-13. **Mandelbrot Image Generator** (Essence Computing) is $9.99 with 4000×4000 output, updated 2024-10-15. **Fracton** is free, updated 2026-09-08. **Beauty of Fractals** (Lutz-R. Frank) is free, v2.13 updated 2026-07-30. — [Mac search results](https://itunes.apple.com/search?term=fractal&country=us&entity=macSoftware&limit=50); [Lookup 6445924588](https://itunes.apple.com/lookup?id=6445924588&country=us)

**2025–2026 newcomers and relaunches (most relevant for almondbread)**

- **Mandelbrot Metal** (Michael Stebel). iPhone, iPad, Mac (M1+), Vision; requires OS 26. First release 2025-10-13. v3.0 shipped 2026-09-22, followed by 3.0.1 (Oct 1) and 3.0.2 (2026-10-12). The release cadence is very high, with roughly 25 versions between Feb and Oct 2026. Model: **freemium with a subscription since v3.0**: Pro Monthly $2.99, Pro Annual $19.99, Pro Lifetime $39.99. Earlier buyers receive "Legacy Pro". It was previously a paid app; search snippets show $2.99 and $0.99 at different times. US 4.79★ / 28. Genres: Graphics & Design, Education. Website mandelbrot-metal.com. — [Lookup 6749807290](https://itunes.apple.com/lookup?id=6749807290&country=us); [App Store page](https://apps.apple.com/us/app/mandelbrot-metal/id6749807290); [search snippet showing earlier paid price](https://apppricinglab.com/app/apple/6749807290)
- **Fractal Dive** (Ronald Kuiper; newlin.nl). iPhone, iPad, Mac, Vision. v1.0 released 2026-09-20. v1.1 (Oct 3) made the app free. v1.2 shipped 2026-10-12. Pro is a one-time $1.99 IAP (Family Sharing; no subscription). Size 2.7 MB. It has no ratings yet. Category: Entertainment. — [Lookup 6813485724](https://itunes.apple.com/lookup?id=6813485724&country=us); [App Store page](https://apps.apple.com/us/app/fractal-dive/id6813485724)
- **Fractal Explorer: Deep Zoom** (Light Year Software, LLC). iPhone/iPad/Mac; requires OS 26. Paid $2.99. Released 2026-08-03; v1.1.1 shipped 2026-10-08. US 5★ / 1. Size 4 MB. — [Lookup 6794896393](https://itunes.apple.com/lookup?id=6794896393&country=us)
- **Ultra Mandelbrot: Fractal Zoom** (Jacob Bartlett, author of the "Jacob's Tech Tavern" blog). iPhone/iPad. Free. Released 2026-09-20; v1.1 shipped 2026-09-22. 259 MB, likely because it ships precomputed data for its curated destinations (inference). No ratings yet. — [Lookup 6811671572](https://itunes.apple.com/lookup?id=6811671572&country=us)
- **MandelKit: Fractal Explorer** (Bjoern Kindler). Universal Mac/iPhone/iPad. Paid $2.99. Released 2026-08-04; v1.1.3 shipped 2026-09-02. No US ratings yet. — [Lookup 6795585522](https://itunes.apple.com/lookup?id=6795585522&country=us)
- **xFractal** (Jean Bovet). iPhone/iPad/Mac. Free and open source (github.com/jean-bovet/xFractal). First released in 2008; v2.0 on 2026-10-02 is "a complete rewrite… GPU rendering, deep zoom… journey replay". US 3.78★ / 9, mostly from old reviews of the 2008-era CPU version. — [Lookup 300642264](https://itunes.apple.com/lookup?id=300642264&country=us)
- **Unlimited Fractals** (The Virtually Unlimited Corp). Universal. Paid $2.99, "no subscription". Released 2026-07-30. — [Lookup 6792814308](https://itunes.apple.com/lookup?id=6792814308&country=us)
- **Fractal Atlas** (Kristopher John). Free, no ads or tracking. Released 2026-10-05; v1.18.2 shipped 2026-10-11. 5★ / 2. — [Lookup 6816481394](https://itunes.apple.com/lookup?id=6816481394&country=us)
- **Mandelbrot Flight Simulator** (John Ferrara). Free. Released 2026-04-05. — [Lookup 6760922593](https://itunes.apple.com/lookup?id=6760922593&country=us)
- **Infinite Fractal Zoom** (Thomas Richardson). Free with IAP/subscription for "visual packs". It is a music visualizer with Apple Music integration. v3.2 shipped 2026-07-28. — [Lookup 6768128610](https://itunes.apple.com/lookup?id=6768128610&country=us)
- **Mandel-Zoom** (Tino Bahr). $0.99. Released 2025-12-08. — [Lookup 6756220762](https://itunes.apple.com/lookup?id=6756220762&country=us)
- **Mandlebrot Infinite** (Dylan Evan Stone). Free; Rust engine. Released 2026-04-21. 5★ / 3. — [Lookup 6762680008](https://itunes.apple.com/lookup?id=6762680008&country=us)
- **Mandelbrot & Julia Explorer** (IN.TRA.M s.r.o). Free. Released 2026-02-20. 2★ / 1. — [Lookup 6759226200](https://itunes.apple.com/lookup?id=6759226200&country=us)
- Other 2026 entries with zero or near-zero ratings: Fractalicious (2026-10-01), Fractal Navigator (2026-09-14, includes Watch), Infinomatic (2026-09-04), Fractal Lens (2026-08-17), Seethe: Fractal Art ($3.99, 2026-09-24), Fractal Universe ($1.99, 2026-01-06), benoît M (2026-06-25), Uzumaki (2026-01-08), Fractals Julia ($0.99), Mandelbrot Shot (a game), Fractal Travel, DoomFracting, Argandly. — [Search results, fractal](https://itunes.apple.com/search?term=fractal&country=us&entity=software&limit=50); [Search results, mandelbrot](https://itunes.apple.com/search?term=mandelbrot&country=us&entity=software&limit=50)

**Requested apps not found**

- **Mandelbrot Maps** is a University of Edinburgh (Philip Wadler) project. It exists as an Android app (v1.03, 2015, on Google Play and F-Droid) and as a 2019 web app. No iOS or Mac App Store listing turned up in the iTunes API or in web search. — [Mandelbrot Maps homepage](https://homepages.inf.ed.ac.uk/wadler/mandelbrot-maps/); [F-Droid](https://f-droid.org/packages/uk.ac.ed.inf.mandelbrotmaps); [web search](https://apps.apple.com/us/app/mandelbrot-navigator-2/id1643541730?uo=4)
- **Fractile**: no App Store app by this name was found. The nearest match is "Fractiles" (Ejago Inc.), a free 2025 game in the Games category that is unrelated. — [Search results, "fractile"](https://itunes.apple.com/search?term=fractile&country=us&entity=software&limit=30)
- Mandelbulb/3D apps on iOS are scarce and mostly stale. The options are Mandelbulb Raytracer (2019), Magic Fractals & Shapes 3D (2021), Fractal 3D (2015), and Unlimited Fractals (2026), which includes ray-marched Mandelbulb, Mandelbox, Menger and Quaternion Julia. MandelKit and Mandelbrot Metal Pro offer "3D relief/lighting" height-field shading, not true 3D. — [Lookup 347888739](https://itunes.apple.com/lookup?id=347888739&country=us); [Lookup 6792814308](https://itunes.apple.com/lookup?id=6792814308&country=us); [Lookup 6795585522](https://itunes.apple.com/lookup?id=6795585522&country=us)

### Inferences
- The category's rating volumes are tiny (Frax has 232 US ratings after 13 years). Even if each rating stands for 50–100 downloads, a rough rule of thumb and an estimate only, the whole Apple fractal-explorer niche is likely in the tens of thousands of active users. It is not a large market.
- Frax's low count relative to its fame may reflect a ratings reset or the shift to free + IAP. This is unverified.
- The burst of 2026 launches, several of them clearly solo or AI-assisted developers with very fast version cadences, means almondbread will face 5–10 look-alike "perturbation + dive + share link" apps at launch. Differentiation will have to come from perceived quality (120 Hz smoothness, depth while moving) and polish rather than from the feature checklist.

### Gaps
- Exact IAP prices for Mandel Draw, Fraksl, Fractally and Infinite Fractal Zoom were not fetched.
- Rating counts for Mac-only apps (Fractal Architect 5/X, JWildfire, MandArt, Mandelbrot Image Generator) are not available from the API.
- No download or revenue estimates were obtained. Sensor Tower and Appfigures public pages were not fetched.

## Rendering tech and limits (GPU/Metal, max depth, perturbation, real-time vs progressive, frame rate)

### Takeaway
Frax, the incumbent, is hard-capped at double precision (about 2e14, "a trillion-to-one"), and its own help page says the 64-bit limit is "absolute and final". By contrast, at least six newer apps claim deep zoom beyond double precision, and several name perturbation explicitly:
- Fractal Dive: tested to 4e331
- Ultra Mandelbrot: past 10^10000
- Retinamandelbrot: 10^20000
- Mandelbrot Metal 3.0: GPU perturbation
- xFractal 2.0: K. I. Martin perturbation
- Fractal Explorer: past 1e30

None of them publicly claims 120 fps real-time at extreme depth. The only explicit frame-rate claim found was "up to 60 fps", from Mandelbrot Flight Simulator, which is a shallow-zoom app.

### Cited Findings
- **Frax**: "a depth range of a trillion-to-one" (store description). Its help page says that on iPhone you can tap-zoom "roughly 30 times… before you reach the 64bit precision limit… about two hundred thousand billion times magnified", and that the "64 bit limit on iOS is absolute and final". It uses both CPU and GPU, and the image is "constantly re-evaluated" during animation. — [Lookup 568827824](https://itunes.apple.com/lookup?id=568827824&country=us); [Frax help: Explore](https://fr.ax/help/explore)
- **Fractal Dive**: "One orbit is computed on the processor using numbers of arbitrary length; the graphics chip then computes only the differences from it. Tested to four times ten to the power 331." The listing says "Nearly every fractal app stalls around a thousand trillion times magnification." It supports three families (z², z³, a mirrored variant) "all equally deep". — [Lookup 6813485724](https://itunes.apple.com/lookup?id=6813485724&country=us)
- **Mandelbrot Metal 3.0**: "brings GPU perturbation rendering to deep Mandelbrot exploration. Adaptive precision, responsive gesture previews, and center-out refinement" (progressive). Depth and speed are the same for Free and Pro. An earlier version claimed zoom to 10^16× (from a search snippet). — [Lookup 6749807290](https://itunes.apple.com/lookup?id=6749807290&country=us); [snippet](https://apppricinglab.com/app/apple/6749807290)
- **Ultra Mandelbrot**: "uses Metal and high-precision calculations to reveal detail at zooms past 10¹⁰⁰⁰⁰×… Very deep views take longer to calculate", and "Maths computations are saved to return to your saved zooms ultra-fast". — [Lookup 6811671572](https://itunes.apple.com/lookup?id=6811671572&country=us)
- **Fractal Explorer: Deep Zoom**: "Ordinary fractal apps stop at about a million times magnification… This one keeps going, past a million trillion trillion times" (about 1e30). It offers a "Fly" mode that "falls for as long as you let it", and bookmarks "thirty digits deep". "The image sharpens as you hold still and keeps up while you move" (progressive). Recoloring is instant without recompute. — [Lookup 6794896393](https://itunes.apple.com/lookup?id=6794896393&country=us)
- **MandelKit**: native Metal, "arbitrary-precision deep zoom", "progressive rendering", and automatic 8/10/16-bit float colour buffers. On Mac it can export disk-backed tiles up to 100,000×100,000. — [Lookup 6795585522](https://itunes.apple.com/lookup?id=6795585522&country=us)
- **Retinamandelbrot**: "Seamlessly zooming… up to astonishing factors of 10^20000 and beyond". A US review (2025-08-15) reports "zooms over 10^100 at nearly realtime without hiccups". Another US review (2025-08-22) says it "broke at e126" on iPad. — [Lookup 1509664268](https://itunes.apple.com/lookup?id=1509664268&country=us); [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1509664268/sortby=mostrecent/json)
- **xFractal 2.0**: "Deep zoom uses K. I. Martin's perturbation technique"; it magnifies "more than a trillion times" (a conservative claim). — [Lookup 300642264](https://itunes.apple.com/lookup?id=300642264&country=us)
- **Unlimited Fractals**: "precision grows with how deep you go" (exact decimal coordinate entry). **Mandlebrot Infinite**: Rust engine, "precision scales automatically". **Mandel-Zoom**: "Unlimited zoom". None of these states a depth figure. — [Lookup 6792814308](https://itunes.apple.com/lookup?id=6792814308&country=us); [Lookup 6762680008](https://itunes.apple.com/lookup?id=6762680008&country=us); [Lookup 6756220762](https://itunes.apple.com/lookup?id=6756220762&country=us)
- **Fractal Atlas**: "trillions of times closer", meaning double-precision class. — [Lookup 6816481394](https://itunes.apple.com/lookup?id=6816481394&country=us)
- **Mandel Draw**: optional "DF64 operation: Double the float operation to make it possible to zoom in more" (double-float emulation, roughly 1e28 class; the depth figure is an inference). — [Lookup 1547068084](https://itunes.apple.com/lookup?id=1547068084&country=us)
- Shallow apps: **Mysteries of Fractal** ("magnification up to 20,000 times"), **Mandelbrot & Julia Explorer** ("Deep zoom up to 100,000×"), and **Fractally** ("over 1,000,000,000,000 x"). — [Lookup 1086527481](https://itunes.apple.com/lookup?id=1086527481&country=us); [Lookup 6759226200](https://itunes.apple.com/lookup?id=6759226200&country=us); [App Store FR Fractally](https://apps.apple.com/fr/app/fractally/id1534380451)
- **Mandelbrot Flight Simulator**: "GPU-accelerated rendering at up to 60 fps". This is the only explicit fps claim found. — [Lookup 6760922593](https://itunes.apple.com/lookup?id=6760922593&country=us)
- **Fractal Architect** (iOS): "Metal GPU Accelerated. 10 to 20x faster rendering than on the device's CPU" (flame fractals; depth is not applicable). — [Lookup 1445109024](https://itunes.apple.com/lookup?id=1445109024&country=us)
- A 2017 GB review of Fraksl asked for an "Option for uncapped fps please… iPad Pro 2017 with 120hz", which shows that some users notice the 120 Hz cap. — [Reviews GB 1238167366](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=1238167366/sortby=mostrecent/json)

### Inferences
- "Deeper than 1e14" is no longer a differentiator; it is table stakes among 2026 entrants. Sustained 120 Hz motion at 1e1000+ (not "sharpens when you hold still") is still unclaimed by any listing, so that claim, if true and demonstrable in a video preview, is the sharpest positioning hook.
- Fractal Dive's description ("Nearly every fractal app stalls around a thousand trillion…") uses almost the same marketing framing almondbread would use. Expect copy collisions.

### Gaps
- No independent benchmarks (fps, time-to-render at 1e100/1e1000) exist for any of these apps. All depth claims are developer self-reports, except the few user reviews quoted.

## Features: video export, image resolution, sharing/links, iCloud, palettes, 3D, Julia, education, controls

### Takeaway
Each almondbread headline feature is already offered by at least one competitor:
- Autopilot/dive: FractRoam, Fractal Dive, Ultra Mandelbrot, Fractal Explorer, Retinamandelbrot, Mandelbrot Flight Simulator
- Exact-coordinate share links: Frax, FractRoam, Fractal Dive, Fractal Atlas, Retinamandelbrot (`rmb://`), Unlimited Fractals, MandelKit
- Zoom-video export: Fractal Dive Pro (1080p/4K), Retinamandelbrot, Frax Pro (1080p), xFractal (replay)
- Educational framing: Mysteries of Fractal, Fractal Dive's "smaller than an atom" scale cues, Fractal Atlas, Mandelbrot Flight Simulator

No single app combines all of them at proven high quality. Controller support is rare: only Fraksl advertises a gamepad.

### Cited Findings
- **Frax**: multi-touch plus tilt-to-steer flight. Pro unlocks "almost 100 parameters". It renders 200 MP images via cloud IAP; the help page lists 3.1 MP and 12.6 MP for all users and 50.3 MP for Pro, with 81× supersampling. Video recording requires Pro. "Live preset link" sharing. Two light sources and textures. — [Lookup 568827824](https://itunes.apple.com/lookup?id=568827824&country=us); [Frax help](https://fr.ax/help/explore)
- **Fractal Dive**: "Dive" autopilot finds "the richest part" and travels there "for minutes". Free share-a-place link ("whoever opens it lands exactly there, same colours"). Pro offers 4K/8K stills and dive video re-rendered at 1080p/4K, 9 colour schemes (3 free), and colouring by iteration, distance and orbit trap. It shows depth analogies ("like a microscope, smaller than a cell, smaller than an atom"). Keyboard, scroll wheel and trackpad on Mac. — [Lookup 6813485724](https://itunes.apple.com/lookup?id=6813485724&country=us); [App Store page](https://apps.apple.com/us/app/fractal-dive/id6813485724)
- **Mandelbrot Metal**: 156 palettes (55 "Ultra-Wide") and 50+ bundled bookmarks. Free includes 5 personal bookmarks and PNG at canvas size. Pro adds unlimited bookmarks, photo-derived and custom palettes, 3D lighting, and 4K/6K/8K/custom PNG. Julia for any point, back/forward history, bookmark file exchange, 16 languages. — [Lookup 6749807290](https://itunes.apple.com/lookup?id=6749807290&country=us)
- **Fractal Explorer: Deep Zoom**: 5 families (Mandelbrot, Burning Ship, Tricorn, Celtic, Perpendicular) at powers 2–6, plus Julia via long-press. 9 palettes and 4 colouring modes, including distance estimate. "Fly" to bookmarks from the full view. 11 named landmarks. — [Lookup 6794896393](https://itunes.apple.com/lookup?id=6794896393&country=us)
- **Ultra Mandelbrot**: 100+ curated destinations, auto deep zoom, 16 animated "looks" (Metal, Water, Retro CRT), photos saved with exact locations, and a gallery. Offline, no ads or analytics. — [Lookup 6811671572](https://itunes.apple.com/lookup?id=6811671572&country=us)
- **MandelKit**: Mandelbrot, Julia, Burning Ship, Multibrot/Multijulia/Multicorn. Orbit traps, domain colouring, field lines, AO, height-field 3D relief. iCloud sync, public/private link sharing. PNG/JPEG/TIFF export; 100k×100k tiled export on Mac. In-app manual. — [Lookup 6795585522](https://itunes.apple.com/lookup?id=6795585522&country=us)
- **Retinamandelbrot**: autozoom, real-time Julia morphing, custom shader, high-res image and video export, `rmb://` position links. — [Lookup 1509664268](https://itunes.apple.com/lookup?id=1509664268&country=us)
- **FractRoam**: "Roam autopilot… continuously seeks out the most visually interesting regions", 6 palettes plus colour cycling, a share picture plus a link to the exact spot, and a macOS screensaver. — [Lookup 1043441882](https://itunes.apple.com/lookup?id=1043441882&country=us)
- **Fractal Atlas**: 23 fractals, including L-system/IFS curves and Barnsley fern. Mandelbrot/Julia split view. A plain-language "info card" explains zoom depth. Share link. — [Lookup 6816481394](https://itunes.apple.com/lookup?id=6816481394&country=us)
- **Unlimited Fractals**: broadest family coverage: escape-time, IFS, L-systems, strange attractors, and 3D (Mandelbulb, Mandelbox, Menger, Quaternion Julia). Exact-coordinate bookmarks, shareable locations, no network. — [Lookup 6792814308](https://itunes.apple.com/lookup?id=6792814308&country=us)
- **Mandelbrot Flight Simulator**: live companion Julia while dragging, autopilot "preprogrammed flythroughs", 5 view modes, and an explanatory "What you're seeing" section. — [Lookup 6760922593](https://itunes.apple.com/lookup?id=6760922593&country=us)
- **Mysteries of Fractal**: three chapters of interactive teaching on Julia sets, plus a Creative Mode. Reviewers praise the explanations (US 2020-03-07: "Presented imaginary numbers clearly"). — [Lookup 1086527481](https://itunes.apple.com/lookup?id=1086527481&country=us); [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1086527481/sortby=mostrecent/json)
- **Mandel Draw**: 80 functions, 20+ palettes, iCloud sync of favourites and palettes, 4× high-res share, and a coordinate/magnification overlay. — [Lookup 1547068084](https://itunes.apple.com/lookup?id=1547068084&country=us)
- **Fraksl**: gamepad, MIDI, audio, motion modulators, NDI streaming, and video recording with mic audio. — [Lookup 1238167366](https://itunes.apple.com/lookup?id=1238167366&country=us)
- **xFractal 2.0**: "Replay: watch your whole exploration animate back as a short movie". — [Lookup 300642264](https://itunes.apple.com/lookup?id=300642264&country=us)

### Inferences
- Fractal Dive (free + $1.99 Pro, universal, perturbation, Dive autopilot, share links, 4K dive video, educational scale cues) is the closest functional analogue to almondbread's plan. It launched in September 2026 and is the most important new competitor to track.
- Mandelbrot Metal is the only deep-zoom app found that has moved to a subscription ($2.99/mo, $19.99/yr, $39.99 lifetime). Most 2026 entrants advertise "no subscription", which suggests the market norm is free + one-time unlock, or $1–3 paid.

### Gaps
- Whether any app supports game controllers or a keyboard for Mandelbrot flight (other than Fraksl's gamepad and Fractal Dive's Mac keyboard/trackpad) was not established.
- Video export resolutions for Retinamandelbrot are unknown.

## Recurring praise and complaints in App Store reviews

### Takeaway
The dominant complaint across older apps is **depth limits and pixelation**. Frax is criticised for it across storefronts and years. Crashes, recording bugs, low video quality, broken IAP flows, abandoned apps and tedious onboarding recur. Praise centres on beauty, relaxation and stress relief, speed, and "infinite zoom" when an app actually delivers it. One report of battery drain was found, on Retinamandelbrot.

### Cited Findings
- **Depth limits (Frax):**
  - US 2023-08-27: "I cant infinitely zoom :(".
  - GB 2021-03-06: "The whole point of fractals is infinite zoom… it stops asking you to pinch to zoom out".
  - DE 2022-12-05: "die 'Unendlichkeit' [stößt] hier recht schnell an ihre Grenzen".
  - DE 2023-11-21: "Please unlock infinity zoom !".
  - DE 2024-11-15: "Man kann garnicht weit reinzoomen… das ist doch das wichtigste".
  - Sources: [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Reviews GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Reviews DE](https://itunes.apple.com/de/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json)
- **Depth limits (others):**
  - FractRoam: US 2018-11-27 "once you zoomed all the way in the gets very pixelated"; US 2017-10-04 "limited depth | what's the point?".
  - Mandelbrot Trot: US 2024-09-02 "Can barely zoom to see anything new"; GB 2024-03-11 "You can't zoom in amazingly deeply".
  - xFractal (old version): US 2017-03-25 "as deeply as a cheap 1950s toy microscope".
  - Sources: [FractRoam US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1043441882/sortby=mostrecent/json); [Mandelbrot Trot US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1479289927/sortby=mostrecent/json); [Mandelbrot Trot GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=1479289927/sortby=mostrecent/json); [xFractal US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=300642264/sortby=mostrecent/json)
- **Video export quality/crashes (Frax):**
  - US 2023-08-31: "if you want to post the recorded videos… the video will be super pixelated".
  - GB 2022-02-15: "you can only record video in 1080p".
  - US 2023-12-29: "Every time I press record video the app crashes".
  - US 2023-04-30: they "still haven't added a GIF option".
  - Sources: [Frax US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Frax GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json)
- **IAP / credits friction (Frax):**
  - GB 2021-05-01: "When ever you buy render credits Apple Pay take the money, but nothing shows".
  - US 2025-03-07: "Cannot purchase credits - link broken… Support very fast response".
  - DE 2022-06-06: Pro not appearing on a second iPhone.
  - Sources: [Frax GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Frax US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Frax DE](https://itunes.apple.com/de/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json)
- **Onboarding/controls (Frax):**
  - US 2023-03-07: "the into is so stressful and tedious".
  - DE 2024-11-15: "Sehr nerviges Tutorial".
  - US 2026-07-15: "the tilt controls go crazy… Is there really no way to disable the gyroscope".
  - DE 2021-09-13: on Mac the trackpad makes the flight "völlig durchdreht".
  - Source: [Frax US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Frax DE](https://itunes.apple.com/de/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json)
- **Retinamandelbrot (the closest prior deep-zoom app):**
  - Praise:
    - US 2025-08-15: "zooms over 10^100 at nearly realtime".
    - US 2025-09-20: "probably the best Mandelbrot browser on iOS but it takes up a lot of battery".
    - DE 2024-05-22: "Die beste kostenlose Mandelbrot App".
  - Complaints:
    - US 2025-10-10: "Very buggy. Saving images sometimes works".
    - US 2026-05-08: "the app unceremoniously and silently crashes".
    - GB 2026-07-12: "zoom videos… as big as a gigabyte".
    - DE 2025-10-02: images saved only at 1:1 and "1,55 GB" for 4 saves.
    - US 2025-08-30: "it spins when u zoom for no reason".
  - Feature requests:
    - US 2026-02-11: "Kalles Fraktaler styled color system".
    - US 2026-03-06: Burning Ship, Celtic, Buffalo and other formulas.
    - GB 2021-08-28: show a+bi coordinates.
  - Sources: [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1509664268/sortby=mostrecent/json); [Reviews GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=1509664268/sortby=mostrecent/json); [Reviews DE](https://itunes.apple.com/de/rss/customerreviews/page=1/id=1509664268/sortby=mostrecent/json)
- **Mandel Draw:**
  - US 2026-05-13: "It started lagging when you zoom in… You end up getting pixels".
  - US 2025-11-07: "keeps crashing won't go pass the loading screen".
  - US 2025-03-27: "Crashes when I try to open it".
  - Repeated requests for custom equations and "perp burning ship" (2026-04/05/06).
  - Source: [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1547068084/sortby=mostrecent/json)
- **Praise themes (relaxation, beauty, classroom use):**
  - Frax GB 2021-02-15: "very good app for stress and anxiety".
  - Frax DE 2023-10-19: a teacher has used it "Seit einigen Jahren im Unterricht" with a projector and music for grades 1–9.
  - Fraksl GB 2022-07-26: "This 'game' saved me from a bad trip".
  - Sources: [Frax GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Frax DE](https://itunes.apple.com/de/rss/customerreviews/page=1/id=568827824/sortby=mostrecent/json); [Fraksl GB](https://itunes.apple.com/gb/rss/customerreviews/page=1/id=1238167366/sortby=mostrecent/json)
- **Mandelbrot Metal** (only 4 text reviews, all 5★): US 2025-12-13: "Speed and intuitive UI… zoom in to infinity (and beyond)". US 2026-09-30 (v3.0): "Fast. Pretty. Fractals". No review yet reacts to the v3.0 subscription change. — [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=6749807290/sortby=mostrecent/json)
- **Abandonware and paid-app disappointment:**
  - Mandelbulb Raytracer US 2018-05-05: "all it does is crash… Support is hopeless since the linked site is no longer".
  - Mandelbulb Raytracer US 2019-04-18: "no possible way of export… Why $.99?".
  - Mysteries of Fractal US 2026-05-28: "power slider is missing!".
  - Sources: [Mandelbulb Raytracer US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=347888739/sortby=mostrecent/json); [Mysteries of Fractal US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1086527481/sortby=mostrecent/json)
- No reviews complaining about ads were found among the apps sampled. Most fractal apps are ad-free, and several 2026 listings advertise "no ads, no account". — [Lookup 6816481394](https://itunes.apple.com/lookup?id=6816481394&country=us); [Lookup 6811671572](https://itunes.apple.com/lookup?id=6811671572&country=us)

### Inferences
- User demand is clear: unlimited depth that stays sharp, more formulas (Burning Ship, Celtic, Perpendicular variants, custom equations), shareable/postable video at social-media quality, small files, and coordinate display. Retinamandelbrot shows that a free, unmaintained deep-zoom app can still earn "best on iOS" reviews, which points to unmet demand among deep-zoom enthusiasts. Many of these reviewers appear to be young users who compare apps to Roblox fractal games (Mandel Draw US 2025-05-28) and to YouTube deep zooms (the "Orson Wang's e275 zoom" reference).

### Gaps
- The RSS feed caps at about 50 recent reviews per storefront, so older review history (e.g., Frax 2013–2019) was not sampled.

## Chart rankings, category placement, editorial features

### Takeaway
Fractal apps cluster in **Graphics & Design**, with secondary categories in Entertainment and Education. No evidence was found of any fractal explorer currently charting, receiving an Apple Design Award, or being App of the Day. Frax's only cited award is a 2013-era Macworld Editors' Choice.

### Cited Findings
- **Primary categories:**
  - Graphics & Design: Frax, Fraksl, Mandelbrot Metal, Fractal Explorer, MandelKit, Ultra Mandelbrot, Fractal Atlas, Unlimited Fractals, Fractal Architect.
  - Education: Mandel Draw, Mysteries of Fractal, Retinamandelbrot, xFractal, Mandelbrot Trot.
  - Entertainment: FractRoam, Fractal Dive, Mandelbrot Flight Simulator, Infinite Fractal Zoom.
  - Lifestyle: TeraFractal.
  - Sources: [Lookup batch](https://itunes.apple.com/lookup?id=568827824,1238167366,6749807290,6794896393,6795585522,6811671572,1547068084,1086527481,1509664268,300642264,1043441882,6813485724,6760922593,451021057&country=us)
- Frax's description cites "Macworld Editors' Choice Award Winner!", plus press quotes from Smithsonian ("Immersive"), CNET ("Spectacular") and Macworld ("Exquisite"). Macworld's launch coverage dates from 2013. A web search found no evidence of an Apple App of the Day or Editors' Choice for Frax. — [Lookup 568827824](https://itunes.apple.com/lookup?id=568827824&country=us); [Macworld review](https://www.macworld.com/article/221898/frax-for-ios-review-reveling-in-mandelbrots-and-julias-anywhere.html); [Macworld launch article](https://www.macworld.com/article/221256/fractal-kings-return-with-a-breathtaking-ios-app.html); [Smithsonian](https://www.smithsonianmag.com/science-nature/a-new-app-turns-fractals-into-ornate-art-8346011/)
- Frax's App Store page shows no chart rank. — [App Store page](https://apps.apple.com/us/app/frax/id568827824)
- A Mysteries of Fractal reviewer asked in 2020 "Why isn't this an editor's choice?", which implies it was not featured. — [Reviews US](https://itunes.apple.com/us/rss/customerreviews/page=1/id=1086527481/sortby=mostrecent/json)

### Inferences
- With rating counts this low, no fractal app is likely to rank in the top 200 of Graphics & Design or Education. Editorial featuring, such as a "Math is beautiful" story or App of the Day, is a more realistic visibility lever than organic charting.

### Gaps
- Current chart positions were not retrieved; the Appfigures, Sensor Tower and AppAgg pages were not fetched within the time budget. No MacStories, 9to5Mac or TouchArcade coverage of the 2025–2026 entrants was found. No Reddit threads specific to iOS deep-zoom apps surfaced in search.
