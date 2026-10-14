# Fractal, mathematical-world, scale-shifting and exploration games: survey for almondbread game mechanics (state: 2026-10-14)

Method note: Steam figures (price, release date, review totals, % positive) were pulled live on 2026-10-14 from the Steam storefront APIs `store.steampowered.com/api/appdetails?appids=<id>` and `store.steampowered.com/appreviews/<id>?json=1&language=all&purchase_type=all` (all languages, all purchase types). They differ slightly from the "Steam rating" shown on the store page, which uses a smaller window. Sales estimates below are **my own Boxleiter-style estimates** (Steam review count x 30 to 60 owners per review, a widely used rule of thumb). They are rough, ignore other platforms, and are labelled "est." wherever used. SteamSpy is quoted where available; Gamalytic and VG Insights pages were not fetched in this run.

## 1. Which games use fractals or the Mandelbrot set as the world or core mechanic?

### Takeaway
Fractal games are a small niche. Exactly one game has clear commercial traction: **Fractal Block World** (1,652 reviews, 98% positive, about 50k to 100k owners), a surreal "shrink infinitely" exploration game. The others are free tech demos (Marble Marcher), tools sold as games (Fractal Alchemist, MandelCraft), or weak free flyers (Fractal Fly). I found **no commercial game built on deep-zoom 2D Mandelbrot exploration**. The only "Mandelbrot GeoGuessr" is a free 2025 game-jam entry (MandelMatcher). The adjacent "maths/impossible space" games sell much better when the maths serves puzzles or a mystery: Superliminal (about 34k reviews), Manifold Garden (8.6k), Hyperbolica (1.3k).

### Cited Findings

**Fractal games (world or core mechanic)**

| Game | Platform / price | Release | Steam reviews (all langs) | Core mechanic |
|---|---|---|---|---|
| Fractal Block World (appid 1540570) | Windows; $6.99 | 18 Oct 2021 (after early access) | 1,652, 98.2% positive, "Overwhelmingly Positive" | "Surreal exploration game. Explore a massive fractal... You can shrink infinitely to go deeper into the fractal. Or you can become larger by growing." |
| Marble Marcher (CodeParade) | itch.io, Windows; name-your-own-price | devlog Jan to Feb 2019 | not on Steam; itch 4.7/5 from 167 ratings | Ray-marched marble physics on fractal surfaces; 24 levels, race to the flag |
| Marble Marcher Community Edition | itch.io (michaelmoroz.itch.io/mmce), open source on GitHub | n/a | not on Steam (no Steam listing found) | Community fork with new features, custom resolutions, better graphics/performance |
| MandelCraft (4081860) | Windows; free, "PRO Pack" DLC $24.99 | 27 Feb 2026 | 20, 90% positive | "A tool for creating and exploring fractal worlds"; genre tags include Animation & Modeling, Design & Illustration |
| Fractal Alchemist (1318030) | Windows; $19.99 | 27 May 2020 | 29, 89.7% positive | "A tool for creating and exploring 3D Fractal Worlds"; Utilities/Design tags |
| Fractal Fly (1860120) | Windows, Mac; free | 10 Feb 2022 | 151, 78.8% "Mostly Positive" | Free flight through 3D fractal landscapes |
| Fractal Chicken (887440) | Windows; free, Early Access | 25 Jul 2018 | 14, 85.7% | Voxel survival/crafting in "infinite voxel worlds, look at fractals" |
| Fractal Foundry (4287960) | Windows | "Coming soon" | 0 | Automation game building "nested factories that scale into recursive systems" |
| VR Fractals (506160) | Windows VR; $3.99 | 26 Jul 2016 | 6, 3 positive | "3D Fractal Viewer in Virtual Reality" |
| RealMandelRender – Fractal Visualizer (5227800) | Windows | announced for 5 Nov 2026 | 0 | Mandelbrot zoom with "132 curated destinations", 18 palettes, image/video export (a viewer, not a game) |
| Mandelarium (3719670) | Windows | TBA | 0 | A "haunting metroidvania"; the name is the only Mandelbrot link in the description |

Sources: Steam API data for each appid — [Fractal Block World store](https://store.steampowered.com/app/1540570), [reviews API](https://store.steampowered.com/appreviews/1540570?json=1&language=all&purchase_type=all); [MandelCraft](https://store.steampowered.com/app/4081860); [Fractal Alchemist](https://store.steampowered.com/app/1318030); [Fractal Fly](https://store.steampowered.com/app/1860120); [Fractal Chicken](https://store.steampowered.com/app/887440); [Fractal Foundry](https://store.steampowered.com/app/4287960); [VR Fractals](https://store.steampowered.com/app/506160); [RealMandelRender](https://store.steampowered.com/app/5227800); [Mandelarium](https://store.steampowered.com/app/3719670). Marble Marcher — [itch.io page](https://codeparade.itch.io/marblemarcher). Community Edition — [CE GitHub](https://github.com/WAUthethird/Marble-Marcher-Community-Edition), [CodeParade itch comments](https://codeparade.itch.io/marblemarcher/comments?before=163).

- SteamSpy estimates Fractal Block World at 50,000 to 100,000 owners, with a peak of 72 concurrent players on its snapshot day; the publisher is "Fractal Block Team LLC" — [SteamSpy](https://steamspy.com/app/1540570). All-time-low price $2.79 (60% off) — [DekuDeals](https://www.dekudeals.com/items/fractal-block-world). Est. (Boxleiter x30 to 60): 50k to 100k, consistent with SteamSpy.
- Fractal Block World history: grew out of Dan Hathaway's Infinifrag 2 / Block Arena codebase (Block Arena around 2008; code moved into FBW around 2012) — [nextquest.dev](https://nextquest.dev/games/infinifrag-2), [Kotaku game page](https://kotaku.com/games/block-arena-1). That suggests about a decade of development before the 2021 release (inference from those dates).
- FBW player praise (Steam reviews, fetched 2026-10-14): a 32 h reviewer compares it to imagining superstructures in pavement cracks; a 22 h reviewer took about 16 h to finish; a 52 h reviewer "beat it 4 times" and is "nowhere near the end"; one reviewer says the developer "updates it for years despite nearly 0 players" and that the trailer is "completely inadequate" — [FBW reviews API](https://store.steampowered.com/appreviews/1540570?json=1&language=english&review_type=positive). Criticism is rare: "the vibe made me actually sleepy" — [negative reviews](https://store.steampowered.com/appreviews/1540570?json=1&language=english&review_type=negative).
- Fractal Fly criticism: "VERY outdated", buggy lighting needing "an epilepsy warning", "flying is janky and the details don't go small enough"; another reviewer: "I got a little sick from playing this" — [Fractal Fly reviews API](https://store.steampowered.com/appreviews/1860120?json=1&language=english).
- Marble Marcher: "no longer in active development"; development continues in the Community Edition — [itch.io](https://codeparade.itch.io/marblemarcher).
- itch.io "mandelbrot" tag (18 entries): almost all are viewers/renderers (PICO-8, TIC-80, WebGL). Two game-like exceptions: **Mandelbrot Lander** (pilot a lander onto the fractal edge) and **MandelMatcher** — [itch.io tag page](https://itch.io/games/tag-mandelbrot).
- **MandelMatcher** (radian628): browser, free, Spring Beaver Game Jam 2025, "Explore the Mandelbrot set to find the pattern you see on the screen!"; no scoring system or daily mode described; status "In development"; no ratings shown — [itch.io](https://radian628.itch.io/mandelmatcher).
- Mobile (iOS): mostly explorers, not games. Mandelbrot Trot (Education category) is the nearest to a game; Fractal Gaze (auto-tour, focus mode) has been removed from the US App Store; Fractally advertises zoom beyond 1,000,000,000,000x; FractRoam has a hands-off roaming mode — [Mandelbrot Trot listing](https://marlvel.ai/api/llm/apps/education/mandelbrot-trot), [Fractal Gaze](https://www.appgoblin.info/apps/6768184555), [Fractally](https://iphoneaddict.fr/apps/divertissement/fractally-897.html), [FractRoam](https://mwm.ai/apps/fractroam/1043441882), [GT Fractals](https://apps.apple.com/us/app/-/id1616502343). (These are aggregator pages, so check them against the App Store.)

**Adjacent maths, scale and exploration games**

| Game | Platform / current price | Steam release | Steam reviews | Est. Steam owners (x30 to 60) |
|---|---|---|---|---|
| Superliminal (1049410) | Win/Mac/Linux; $7.99 (on sale at fetch time) | 5 Nov 2020 (Steam) | 34,364, 94.6% | about 1.0M to 2.1M |
| Manifold Garden (473950) | Windows; $19.99 | 20 Oct 2020 (Steam) | 8,617, 95.2% | about 260k to 520k |
| Hyperbolica (1256230) | Win/Mac/Linux; $12.99 | 14 Mar 2022 | 1,327, 83.0% | about 40k to 80k |
| Everything (582270) | Win/Mac/Linux; $1.49 (on sale) | 21 Apr 2017 | 3,870, 83.1% | about 116k to 232k |
| Outer Wilds (753640) | Windows; $12.49 (on sale) | 18 Jun 2020 (Steam) | 109,245, 95.6% | about 3.3M to 6.6M |
| Proteus (219680) | Win/Mac/Linux; $9.99 | 30 Jan 2013 | 2,812, 76.2% "Mostly Positive" | about 84k to 169k |
| Journey (638230) | Windows; $3.74 (on sale) | 11 Jun 2020 (Steam; PS3 original 2012) | 38,749, 94.1% | about 1.2M to 2.3M |

Sources: [Superliminal](https://store.steampowered.com/app/1049410), [Manifold Garden](https://store.steampowered.com/app/473950), [Hyperbolica](https://store.steampowered.com/app/1256230), [Everything](https://store.steampowered.com/app/582270), [Outer Wilds](https://store.steampowered.com/app/753640), [Proteus](https://store.steampowered.com/app/219680), [Journey](https://store.steampowered.com/app/638230) (each with its `appreviews` endpoint). Prices are as fetched on 2026-10-14 and some were discounted.

- Store descriptions (core mechanic): Manifold Garden — "Escher-esque world of impossible architecture. Witness infinity in first-person... solve physics-defying puzzles"; Superliminal — puzzles "using the ambiguity of depth and perspective" (scale-shifting by forced perspective); Hyperbolica — "Non-Euclidean adventure... games, puzzles, mazes, and secrets"; Everything — "thousands of playable characters, endless exploration... narration from Alan Watts" (scale from galaxies to atoms); Proteus — "soundtrack to your play is created by your surroundings"; Outer Wilds — "open world mystery about a solar system trapped in an endless time loop" — same store pages as above.
- Hyperbolica reviews: praise "geometric visual adventure more than an actual game... a great time simply exploring"; "doesn't really explain the hyperbolic space... watch the video on YouTube"; criticism "View+idea was nice, game was boring" — [Hyperbolica reviews API](https://store.steampowered.com/appreviews/1256230?json=1&language=english).
- CodeParade's Patreon (possibly stale) listed 129 members and $98/month while developing Hyperbolica — [Patreon](https://www.patreon.com/codeparade). CodeParade documented development in a YouTube devlog series — [setsideb.com](https://setsideb.com/tag/hyperbolica/). No sales postmortem was found.
- Everything reviews: "most existential screensaver ever" (positive); "Cute idea but overstays its welcome" (negative, 12 h); "This is not a game, it's a scam" (negative) — [Everything reviews API](https://store.steampowered.com/appreviews/582270?json=1&language=english).
- Proteus reviews: "Beautiful and relaxing, but there's really not much to see or do. Only takes about an hour to finish, and has no save state"; "Meditative exploration game... Zen-like flow state. To be played when you're in the right frame of mind"; negatives are about crashes and bugs on modern systems — [Proteus reviews API](https://store.steampowered.com/appreviews/219680?json=1&language=english).

### Inferences
- The fractal niche on Steam is small: the best fractal title (FBW) has about 1/20 the reviews of Superliminal and about 1/66 of Outer Wilds. The fractal games that succeed are **games with a mystery, progression and secrets** (FBW has lore, an ending and replay depth). Tools and free flyers stall at tens to low hundreds of reviews.
- Fractal *tools* labelled as games (Fractal Alchemist, MandelCraft) gather about 20 to 30 reviews, which supports the existing finding in this KB that tools do not sell on Steam.
- The free 3D fractal flyer that ships with janky flight and limited detail depth (Fractal Fly) is reviewed harshly on exactly the axes where almondbread is strongest (deep zoom, stable rendering). Motion sickness is a real complaint for fractal flight.
- Maths-themed games keep their positive ratings when the maths drives puzzles (Manifold Garden 95%, Superliminal 95%). Hyperbolica, where the maths is mostly the spectacle, sits at 83%.

### Gaps
- No Steam listing for Marble Marcher or its Community Edition was found; ratings exist only on itch.io. Download counts were not visible.
- Gamalytic and VG Insights revenue estimates were not fetched. Boxleiter numbers above are my own and Steam-only. Manifold Garden, Superliminal, Journey and Everything also sold on consoles, Epic or mobile.
- Fractal Block World team size (solo or small team) is unconfirmed.
- MandelCraft review samples timed out. Fractals (3894620) details could not be fetched.
- Apple App Store data came through aggregators, not the App Store itself.

## 2. Has anyone built a game on deep-zoom Mandelbrot exploration specifically? Failed attempts or postmortems?

### Takeaway
No commercial deep-zoom Mandelbrot *game* was found on Steam, itch.io or the App Store as of October 2026. Existing items are viewers (RealMandelRender with curated destinations, coming Nov 2026; many itch.io renderers), a free jam prototype of the "find this pattern" idea (MandelMatcher, 2025), and Mandelbrot Lander. No postmortems exist because nobody has shipped and reported on such a game. The space is open, but it is also unvalidated.

### Cited Findings
- Steam store search for "mandelbrot" returned nothing; "Mandel" returned MandelCraft, RealMandelRender and the unrelated metroidvania Mandelarium — Steam storesearch API (`store.steampowered.com/api/storesearch/?term=Mandel`), and pages [MandelCraft](https://store.steampowered.com/app/4081860), [RealMandelRender](https://store.steampowered.com/app/5227800).
- RealMandelRender: "Zoom freely, explore 132 curated destinations... 18 color palettes. Save your discoveries as images or videos"; release announced for 5 Nov 2026 — [Steam](https://store.steampowered.com/app/5227800). This is a direct competitor to almondbread as a Steam *viewer*, not a game.
- MandelMatcher is a "find the pattern on screen" game, made in a 2025 game jam, free, browser, still marked in development — [itch.io](https://radian628.itch.io/mandelmatcher).
- A web search for a daily Mandelbrot location-guessing game found only explorers (e.g. mandelbrot.page, Mandelbrot Maps) and Mandelprison, an 80s-style text adventure about finding periodicities — [itch.io fractal tag](https://itch.io/games/tag-fractal), [Mandelbrot Maps](https://jmaio.github.io/mandelbrot-maps/), [Mandelprison GitHub](https://github.com/marcm200/Mandelprison).
- Mandelbrot Lander: pilot a lander toward the "Mandelbrot Cloud" and land on the fractal edge — [itch.io tag page](https://itch.io/games/tag-mandelbrot).

### Inferences
- The daily "find this location" idea has been tried at least once, as a jam game, without visible traction. That does not disprove it: there was no daily cadence and no share card, the two parts that made Wordle spread.
- Fractal Block World is the nearest existing thing to "infinite depth as a game world". It succeeded through hand-authored mystery and lore on top of the fractal, not through the fractal alone.

### Gaps
- Searches did not cover r/fractals, r/gamedev or the fractalforums archive in depth for abandoned prototypes. Abandoned deep-zoom game attempts, if any, are undocumented in what I found.
- Whether MandelMatcher has any player counts is unknown.

## 3. What makes exploration/walking-sim/zen games succeed or fail commercially? Evidence for daily-challenge virality and discoverer naming / shared-world collection

### Takeaway
Exploration and zen games are hit by "not a game", "too short" and "overstays its welcome" complaints. The ones that succeed attach a mystery, puzzles or progression (Outer Wilds, FBW, Manifold Garden) or sell cheaply (Everything, Proteus on sale). Daily challenges can spread explosively: Wordle went from 90 to 300,000 players in two months on a spoiler-free share grid. The wave also decays: Heardle fell from about 69M to 41M monthly visits and was then shut down. Naming and discovery credit motivates players (No Man's Sky, Galaxy Zoo's Hanny's Voorwerp, Foldit groups credited as paper authors), but every public naming system attracts offensive names and needs moderation.

### Cited Findings

**Exploration / walking-sim reception**
- Gone Home drew "a massive backlash from consumers who dismissed it as a game entirely and believed it was not worth the twenty-dollar purchase price", while critics scored it highly; "walking simulator" began as a derogatory term — [Gone Home, Wikipedia](https://en.Wikipedia.com/wiki/Gone_Home).
- Steve Gaynor (Fullbright): the biggest criticism was length ("two or three hours... and $20"); they priced it planning for Steam sales, and it was $10 a couple of months after release; 75% of sales came from Steam — reported via secondary coverage in the [NME "Dear Esther at 10" feature](https://www.nme.com/features/dear-esther-at-10-and-the-rise-and-fall-of-the-walking-simulator-3161240) and [Gone Home, Wikipedia](https://en.Wikipedia.com/wiki/Gone_Home) (the original GamesIndustry interview was not fetched).
- Later walking sims did worse: The Chinese Room shut down its studio, and "Tacoma initially sold barely 1% of the copies of Gone Home" — [NME](https://www.nme.com/features/dear-esther-at-10-and-the-rise-and-fall-of-the-walking-simulator-3161240) (secondary claim).
- Review evidence from this run: Proteus at 76% with "not much to see or do... about an hour"; Everything "overstays its welcome" and "not a game, it's a scam"; Hyperbolica "game was boring" — reviews APIs for [Proteus](https://store.steampowered.com/appreviews/219680?json=1&language=english), [Everything](https://store.steampowered.com/appreviews/582270?json=1&language=english), [Hyperbolica](https://store.steampowered.com/appreviews/1256230?json=1&language=english). By contrast, the mystery-driven Outer Wilds holds 95.6% on 109k reviews — [Steam](https://store.steampowered.com/app/753640).
- FBW reviewers praise depth that lasts 16 to 50+ hours and secrets "I might die before I find it all" — [FBW reviews API](https://store.steampowered.com/appreviews/1540570?json=1&language=english&review_type=positive).

**Daily challenge virality**
- Wordle: 90 players on 1 Nov 2021, about 300,000 two months later; players shared "a grid of green, yellow and black boxes" on social media; the NYT bought it in January 2022 for "low-seven figures" — [NPR via WHYY](https://whyy.org/npr-story/the-new-york-times-buys-wordle), [KQED](https://www.kqed.org/arts/13908753/the-new-york-times-buys-wordle).
- Decline: Heardle peaked at about 69M monthly visitors and was at 41M before Spotify bought it, then shut down (offline from 5 May 2023) — [dot.la](https://dot.la/spotify-heardle-shut-down-2659860389.html), [Gizmodo](https://gizmodo.com/spotify-heardle-wordle-music-streaming-1850336570). Google Trends put Wordle at about a third of its peak popularity (secondary, student editorial) — [Seisen Post](https://www.seisen.com/student-life/seisen-post/features/~board/seisen-post/post/why-is-wordle-not-popular-anymore).
- Framed: one movie still per day, five guesses, Wordle-style — [Rolling Stone UK](https://www.rollingstone.co.uk/culture/news/wordle-inspired-game-framed-challenges-film-buffs-13584/). No player numbers found.
- GeoGuessr: reportedly 80M+ players in 140+ countries (growth from 10M in 2019); in early 2024 it moved from freemium to a full subscription, and only the daily challenge stays free — [Dealroom](https://dealroom.co/companies/geoguessr/). It also has a Steam edition, with about 1.6k concurrent and a 3.1k 24-hour peak at the time of the snapshot — [SteamCharts](https://steamcharts.com/app/3478870). There are no official daily-challenge participation figures ("precise player statistics are not publicly released") — [London Design Festival page](https://program.londondesignfestival.com/update2025/news/geoguessr).

**Discoverer naming / shared world / citizen science**
- No Man's Sky: players name and upload discoveries. Hello Games told Kotaku it is "common sadly for the most problematic players to find ways to have the most offensive names"; the studio reports such players to the platform holder and does not publicly discuss troll methods — [Kotaku](https://kotaku.com/no-mans-sky-galactic-hub-bases-civilized-space-neo-nazi-1848739563), [TheGamer](https://www.thegamer.com/neo-nazi-banned-creating-chaos-galactic-hub-no-mans-sky/). At launch (2016) players reported named discoveries disappearing, later attributed to server refresh behaviour — [Shacknews](https://www.shacknews.com/article/96530/no-mans-sky-appears-to-delete-your-discoveries-after-two-weeks-updated), [Gameranx](https://gameranx.com/updates/id/71620/article/is-the-no-mans-sky-naming-deletion-a-bug/). Per a Steam forum claim (unverified), the profanity filter applies only to in-game entry and a save editor can bypass it — [Steam discussion](https://steamcommunity.com/app/275850/discussions/0/592889669725803833).
- Galaxy Zoo: schoolteacher Hanny van Arkel found "Hanny's Voorwerp" in 2007 as a volunteer; the object carries her name, and she is a co-author of the 2009 MNRAS paper — [Wikipedia](https://en.wikipedia.org/wiki/Hanny%27s_Voorwerp), [Astronomy.com](https://www.astronomy.com/science/schoolteacher-discovers-cosmic-ghost/), [Univ. of Hertfordshire repository](https://uhra.herts.ac.uk/handle/2299/4317).
- Foldit: players solved the M-PMV retroviral protease structure in about three weeks; the 2011 *Nature Structural & Molecular Biology* paper lists the "Foldit Contenders Group" and "Foldit Void Crushers Group" as co-authors — [UW News](https://www.washington.edu/news/2011/09/19/gamers-succeed-where-scientists-fail), [Baker Lab](https://www.bakerlab.org/2015/11/26/crystal-structure-of-a-monomeric-retroviral-protease-solved-by-protein-folding-game-players/), [DOI](https://www.doi.org/10.1038/NSMB.2119). A 2010 *Nature* paper credited Foldit players collectively (57,000 players) — [Wikipedia: Foldit](https://en.wikipedia.org/wiki/Foldit).

### Inferences
- "Journey mode" alone is the commercially weakest option. Zen and no-goal games are reviewed down for length and lack of goals, and sell mostly cheaply or on sale. As one mode inside a bigger product, it carries no such risk.
- Daily challenges are cheap and can spread, but the spike decays. They work best as a free acquisition and retention funnel (GeoGuessr keeps the daily free behind a paywall), not as the product.
- Discovery credit is a strong motivator when it is *permanent and public* (Hanny's Voorwerp, Foldit authorship). The NMS experience shows it needs naming moderation (filter, reporting, rename-by-admin) from day one.

### Gaps
- No primary-source peak DAU for Wordle or daily-challenge participation for GeoGuessr.
- EVE Online probe-scanning was not researched in this run.
- No refund-rate data for short exploration games was found.

## 4. Which candidate mechanics have the strongest evidence, and which are risky?

### Takeaway
Strongest evidence: the **daily find-this-location challenge** (Wordle/GeoGuessr pattern, low cost, already prototyped by MandelMatcher with no daily or share layer) and **discoverer naming in a shared atlas** (proven motivator in NMS and citizen science; almondbread's exact coordinates make it unusually verifiable). Medium: **depth missions** (they give the goals whose absence hurts exploration games). Unproven and costly: **canyon racing** (no fractal racing precedent found except Marble Marcher's small free time-trial; fractal flight draws janky-flight and motion-sickness complaints). Risky as a product, fine as a mode: **journey mode** ("not a game", "overstays its welcome").

### Cited Findings
- Daily challenge: Wordle 90 to 300k players in about two months via a share grid — [WHYY/NPR](https://whyy.org/npr-story/the-new-york-times-buys-wordle); GeoGuessr keeps its daily challenge free as its funnel — [Dealroom](https://dealroom.co/companies/geoguessr/); prior Mandelbrot version exists only as a jam game — [MandelMatcher](https://radian628.itch.io/mandelmatcher); clones decay (Heardle 69M to 41M monthly, then closed) — [dot.la](https://dot.la/spotify-heardle-shut-down-2659860389.html).
- Atlas / naming: NMS naming plus offensive-name moderation problem — [Kotaku](https://kotaku.com/no-mans-sky-galactic-hub-bases-civilized-space-neo-nazi-1848739563); naming credit in citizen science — [Hanny's Voorwerp](https://en.wikipedia.org/wiki/Hanny%27s_Voorwerp), [Foldit](https://www.washington.edu/news/2011/09/19/gamers-succeed-where-scientists-fail).
- Depth and goals: the most successful fractal game emphasises secrets, an ending and replayable depth — [FBW reviews](https://store.steampowered.com/appreviews/1540570?json=1&language=english&review_type=positive); goal-less titles draw "not a game" and length complaints — [Gone Home](https://en.Wikipedia.com/wiki/Gone_Home), [Everything reviews](https://store.steampowered.com/appreviews/582270?json=1&language=english).
- Racing / flight: Marble Marcher (fractal time-trial, 24 levels, about 30 minutes average session) is free and abandoned in favour of the CE — [itch.io](https://codeparade.itch.io/marblemarcher); Fractal Fly flight criticised as "janky", plus nausea and lighting/epilepsy complaints — [Fractal Fly reviews](https://store.steampowered.com/appreviews/1860120?json=1&language=english).

### Inferences
- Order recommendation in decisions/gamification.md (daily → atlas → racing) is consistent with the evidence. Racing has the weakest evidence relative to its cost.
- A Steam edition needs a game hook. Tool-like fractal products on Steam get about 20 to 30 reviews; a viewer competitor with curated destinations (RealMandelRender) launches November 2026.
- Motion sickness and flight feel should be a prototype gate for landscape mode and racing (inferred from Fractal Fly reviews; n is small).
- The atlas needs a naming filter and a report path at launch, and a classification that resists trivially near-duplicate "discoveries" (inferred from NMS).

### Gaps
- No evidence on ghost-replay racing in exploration games specifically (e.g. Trackmania-style ghosts) was gathered.
- No data on the retention of daily-challenge features embedded in a paid app (as opposed to free web games).
