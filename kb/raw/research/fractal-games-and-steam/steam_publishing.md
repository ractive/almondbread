# Publishing almondbread on Steam (state: October 2026)

Scope: practical guide for a solo developer weighing a Steam edition of almondbread (deep-zoom fractal explorer; planned daily challenge, discovery atlas, canyon racing with ghosts) for Windows, Linux/Steam Deck and macOS.

Verification legend. **[fetched]** means the page was read during this session (2026-10-14). **[not re-fetched]** means the claim comes from Steamworks docs or policy pages as known up to mid-2026; the URL is given but the page was not re-read this session, so check it before relying on the claim. Inferences are always in the "Inferences" subsections.

---

## 1. Process and costs (onboarding, store page, builds, revenue, refunds, pricing, software vs game, playtests, demos)

### Takeaway
Getting onto Steam is cheap: $100 per app, recouped once the app reaches $1,000 adjusted gross revenue. Valve takes 30% until a title passes $10M, so for almondbread the 25% and 20% tiers are irrelevant. The real constraints are timing. Bank, tax and identity verification takes 10–15 business days, there is a wait of 21 or 30 days after paying the fee (sources disagree), a public Coming Soon page must be up for at least 2 weeks, and each store-page or build review takes 1–5 days. Plan on at least about 6–8 weeks from signup to release, and much longer if you want to collect wishlists first.

### Cited Findings
**Onboarding and fees**
- Onboarding steps: sign the NDA and Steam Distribution Agreement electronically, pay the app fee, then provide bank, tax and identity information. **[fetched]** — [Steamworks: Onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding)
- Identity: you need "accurate, legal identification of the entity signing". The bank account holder name must match the name used at onboarding. The tax questionnaire is a W-9 for US persons or a W-8BEN for eligible foreign persons. Verification "may take 10 to 15 business days". **[fetched]** — [Onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding)
- Steam Direct fee: "$100 USD (or equivalent) fee for each new app". It cannot be refunded, but it is recouped once the product reaches "at least $1,000.00 Adjusted Gross Revenue". It cannot be paid with Steam Wallet funds. **[fetched]** — [Steamworks: App Fee](https://partner.steamgames.com/doc/gettingstarted/appfee); [Steam Direct](https://partner.steamgames.com/steamdirect)
- **Conflict on the waiting period.** The onboarding page as fetched today says "A 21-day waiting period between when you paid the app fee and when you can release that game". A search-engine summary of the same page said 30 days, and a third-party guide also gives 21. **Flag:** re-check the live page; budget for 30 days to be safe. — [Onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding)
- A publicly visible Coming Soon page is required for at least two weeks before release. **[fetched]** — [Onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding)
- Valve reviews the store page and the build separately. Each review "takes between 1-5 days" and checks that they are "configured correctly and running as expected". **[fetched]** — [Onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding)
- The Steamworks "Getting released" hub links separate pages for the Release Process, Review Process, Coming Soon, Demos, Testing On Steam and Steam Playtest. The hub itself contains no details. **[fetched]** — [Steamworks: Store/Get released](https://partner.steamgames.com/doc/store/getreleased)

**Revenue share**
- Valve takes 30% of revenue up to $10M, 25% between $10M and $50M, and 20% above $50M. The tiers took effect for revenue from 1 Oct 2018 and cover packages, DLC, in-game sales and Marketplace fees. I found no 2026 change. — [TechCrunch 2018](https://techcrunch.com/?p=1753729); [MCV/UK](https://www.mcvuk.com/business-news/valves-new-steam-distribution-agreement-lets-big-game-devs-keep-more-of-their-profits/)
- GameDiscoverCo estimates Valve's effective take in 2025 at about 24% across non-Valve games. That average is pulled down by the big titles in the lower tiers. — [GameDiscoverCo: "Steam's 24% cut"](https://newsletter.gamediscover.co/p/revealed-the-numbers-behind-steams)

**Refunds**
- Steam refund policy: any purchase can be refunded within 14 days of purchase if it was played for less than 2 hours. Valve can refuse refunds it considers abuse. **[not re-fetched; the page refused connections this session]** — [Steam Refunds](https://store.steampowered.com/steam_refunds/)
- Steamworks lists a "Developer Refund Reporting" page, so refund data is visible to developers. **[fetched, title only]** — [Onboarding nav](https://partner.steamgames.com/doc/gettingstarted/onboarding)

**Builds (SteamPipe)**
- Builds are uploaded with SteamPipe (the steamcmd ContentBuilder, using app/depot build scripts). Each OS normally gets its own depot, filtered by OS in the depot settings. Builds are assigned to branches: "default" is public, and other branches can be password-protected betas. **[not re-fetched]** — [Steamworks: Uploading to Steam](https://partner.steamgames.com/doc/sdk/uploading); [Branches](https://partner.steamgames.com/doc/store/application/branches)
- On the platforms page, developers declare supported OSes and confirm macOS 64-bit and notarized bundles (see section 4). **[search-snippet]** — [Steamworks: Platforms](https://partner.steamgames.com/doc/store/application/platforms)

**Pricing**
- Steamworks suggests regional prices from a base USD price, and developers can override them per currency. **[not re-fetched]** — [Steamworks: Pricing](https://partner.steamgames.com/doc/store/pricing)

**Software vs game**
- I found no Steamworks page in this session that sets out the rules for classifying an app as Software versus Game. Community threads only confirm that a "Software" category and filter exist and that third parties such as Discord treat Software apps as non-games. — [Steam Community thread](https://steamcommunity.com/discussions/forum/0/3758852682828978299); [Discord feedback thread](https://support.discord.com/hc/en-us/community/posts/360046591771-Detect-Steam-software-as-not-being-a-game)

**Playtests and demos**
- Steam Playtest creates a separate free app linked to the main game. Players request access and the developer grants it in waves. Demos are also separate apps with their own store presence, and they can share Steam Cloud with the full game through "Shared cloud APP ID". **[cloud: fetched; playtest/demo: not re-fetched]** — [Steamworks: Steam Cloud](https://partner.steamgames.com/doc/features/cloud); [Steam Playtest](https://partner.steamgames.com/doc/features/playtest); [Demos](https://partner.steamgames.com/doc/store/application/demos)

### Inferences
- Calendar for a solo developer: in week 0, sign up and pay. Weeks 0–3 go to tax/bank verification (10–15 business days), running in parallel with store page assets. Then comes the store page review (1–5 days) and the Coming Soon page going live. Coming Soon must stay up for ≥2 weeks, but for marketing reasons it should be months (see section 2). The build review (1–5 days) should happen at least about 1 week before the target date. The 21- or 30-day fee wait is almost always shorter than the marketing runway.
- Ship as a **Game**, not Software. The game modes (daily challenge, racing, achievements) justify it, and the Software category gets far less discovery and has no Next Fest culture. This inference comes from the absence of software-specific support in Next Fest rules and the tag and discovery systems, not from an explicit Valve rule.
- With a 2-hour refund window, a contemplative explorer that someone can "finish" in under 2 hours is at refund risk. The daily challenge and atlas progression give players reasons to keep playing past that point.
- Use a password-protected beta branch for testers. Use a Playtest app for broader external testing before the demo.

### Gaps
- The exact current waiting period (21 vs 30 days) conflicts and needs a direct check.
- Official criteria for Software vs Game classification were not found.
- The refund page could not be fetched this session.

---

## 2. Marketing mechanics (wishlists, conversion, Next Fest, tags, visibility, discounts, timing, median outcomes, streamers)

### Takeaway
Wishlists remain the main leading indicator. Median first-week conversion is about 10–20% of wishlists depending on dataset and price, and outcomes vary by an order of magnitude. The median Steam game earns very little, in the hundreds to low thousands of dollars lifetime. How To Market A Game's "Bronze" tier, roughly $1,000 first-week revenue, corresponds to about 5,000 wishlists at launch. Next Fest can be used only once per game, so do not spend it on an early demo.

### Cited Findings
**Wishlist conversion**
- GameDiscoverCo (3 Nov 2025): median first-week conversion is **0.15×** wishlists for games launching with more than 25k wishlists over the past two years, and **0.10×** for games priced above $10. GameDiscoverCo sees no long-term decline in conversion; instead, gathering wishlists is getting harder. Conversion "varies by an order of magnitude" between projects. Underperformers had a 67% median 7-day user score versus 91% for overperformers, and a longer pre-release period (411 vs 214 days on average). — [GameDiscoverCo via GameDevReports](https://gamedevreports.substack.com/p/gamediscoverco-the-state-of-steam)
- How To Market A Game benchmarks (median first-week conversion by wishlists at launch): <5k → 15%, 5k–40k → 20%, 40k–100k → 23%, 100k+ → 25%. The page dates this study to June 2020 (Simon Carless), but a search summary said Feb 2025, so the date is uncertain. By revenue tier: Bronze ($1k) 11.93%, Silver ($10k) 20.49%, Gold ($100k) 25.35%. — [HTMAG Benchmarks](https://howtomarketagame.com/benchmarks/)
- HTMAG wishlists needed at launch (updated June 2026): Bronze 5,000 / Silver 8,000 / Gold 50,000 / Diamond 90,000. — [HTMAG Benchmarks](https://howtomarketagame.com/benchmarks/)
- HTMAG wishlists per week (Aug 2022): Bronze 0–40, Silver 15–120, Gold 100–700. Wishlists gained on the day the Coming Soon page goes live (Sept 2026): Bronze 23–68, Silver 79–441. Wishlist deletes are about 9–12.7%. Median demo playtime (Oct 2022): 7 / 18 / 38 / 65 min by tier. — [HTMAG Benchmarks](https://howtomarketagame.com/benchmarks/)
- Visibility: discovery-queue impressions in the 10 days after launch are about 22k for Bronze (54% of traffic) and 126k for Silver (Feb 2025 study). — [HTMAG Benchmarks](https://howtomarketagame.com/benchmarks/)
- Boxleiter number (sales per review): roughly 31 median for 2020–2021 releases (range 20–55). This is old data. — [HTMAG Benchmarks](https://howtomarketagame.com/benchmarks/)

**Next Fest**
- Official rules (June 2026 edition): you need a public store page and a publicly playable demo when the fest starts. Prologues and chapter-one releases do not count as demos. The game must release after the fest. "Titles are only allowed to participate in one Next Fest." — [Steamworks: Next Fest June 2026](https://partner.steamgames.com/doc/marketing/upcoming_events/nextfest/2026june)
- The October 2026 Next Fest runs 19–26 Oct 2026, and registration closed 31 Aug. The February 2027 edition is reported as 22 Feb–1 Mar 2027, with registration closing 10 Jan 2027. These dates come from secondary sources; verify them on Steamworks Upcoming Events. — [StraySpark](https://www.strayspark.studio/blog/steam-next-fest-october-2026-prep-timeline); [SteamPageAnalyzer Feb 2027](https://www.steampageanalyzer.com/blog/steam-next-fest-february-2027)
- HTMAG Next Fest wishlist tiers (Feb 2026 survey): Bronze 0–999, Silver 1,000–6,999, Gold 7,000–9,999, Diamond 10,000+. — [HTMAG Benchmarks](https://howtomarketagame.com/benchmarks/)

**Median outcomes**
- Median Steam game: about $249 gross in 2025 according to Ziva/Fungies citing VG Insights and Alinea, or about $1,000 lifetime according to SteamPageAnalyzer citing VG Insights via Zukowski. Both are secondary aggregations and disagree, partly because one is a single-year figure and the other is lifetime. — [Ziva](https://ziva.sh/blogs/indie-game-revenue); [SteamPageAnalyzer](https://www.steampageanalyzer.com/blog/median-steam-game-2026)
- 2025 had about 20,282 Steam releases. Nearly half never reached 10 reviews, and about 3% reached 1,000 reviews. These figures are secondary. — [SteamPageAnalyzer](https://www.steampageanalyzer.com/blog/median-steam-game-2026)
- GameDiscoverCo: median price of the top 50 new games fell from $19.50 to $15.64 (−20%) between Feb 2023 and Oct 2025. — [GameDiscoverCo via GameDevReports](https://gamedevreports.substack.com/p/gamediscoverco-people-are-spending)

### Inferences
- Realistic base case for a niche fractal explorer with zen and puzzle elements: a launch in the Bronze-to-Silver range (a few thousand wishlists, about $1k–$10k first week). Treat Steam as a channel worth about $100 plus engineering time, not a business plan, unless wishlist velocity on the Coming Soon page proves otherwise. HTMAG's weekly-wishlist bands are a cheap go/no-go signal: below about 15–40 per week organically means Bronze.
- Fractals are very visual and well suited to clips. Short deep-zoom videos and streamer-friendly racing with ghosts are the most plausible hooks. Tags such as Relaxing, Exploration, Procedural Generation, Abstract, Psychedelic, Atmospheric, Racing and Casual should be tested against comparable games on SteamDB. This is a hypothesis and was not researched in this session.
- Spend Next Fest once, when the demo is polished. The daily challenge plus a single canyon is an ideal demo slice. Do not enter while the game modes are still unfinished.

### Gaps
- No primary data was found on sales for exploration or zen fractal titles specifically. Comparable titles and their review counts would need a SteamDB pass.
- Discount and seasonal-sale mechanics (discount cooldowns, launch discount rules) and streamer/YouTube outreach data were not researched this session.
- The date of the HTMAG wishlist-conversion study conflicts (2020 on the page vs 2025 in a search snippet).

---

## 3. Steamworks features for the planned mechanics; server needs

### Takeaway
Steam alone can cover daily-challenge and race leaderboards, including ghost replays attached to leaderboard entries as UGC. It can also cover cloud saves and bookmarks, sharing locations, flights and palettes through Workshop, and timeline markers. Steam does **not** provide a trusted authority for "first discoverer" claims or tamper-proof scores. Client-written leaderboards are cheatable. Trusted leaderboards require scores to be written through the Web API, which needs a publisher key and therefore your own server. A global first-discoverer atlas needs your own backend.

### Cited Findings
**Leaderboards and ghost replays**
- Up to **10,000 leaderboards per app**. A player has one entry per leaderboard. The score is an int32, with up to **64 int32 "details"** per entry. Upload methods are KeepBest or ForceUpdate. Sort order is ascending or descending, and the display type is Numeric, Seconds or Milliseconds. **[fetched]** — [Steamworks: Leaderboards](https://partner.steamgames.com/doc/features/leaderboards)
- After uploading a score, `AttachLeaderboardUGC` can link UGC "such as a replay or a ghost". Other users get it through `LeaderboardEntry_t.m_hUGC`, and the attachment survives even if the user changes or deletes the underlying Cloud file. The UGC handle must come from `ISteamRemoteStorage::FileShare`. **[fetched + search]** — [Leaderboards](https://partner.steamgames.com/doc/features/leaderboards); [ISteamUserStats](https://partner.steamgames.com/doc/api/ISteamUserStats)
- **Trusted writes:** when the leaderboard "Writes" setting is Trusted, "scores cannot be set by clients", only through the `SetLeaderboardScore` Web API. Reads can be restricted to Friends; the Web API can always read all scores. **[fetched]** — [Leaderboards](https://partner.steamgames.com/doc/features/leaderboards)
- The leaderboards page does not describe resets or a "daily" leaderboard feature. **[fetched]** — [Leaderboards](https://partner.steamgames.com/doc/features/leaderboards)

**Steam Cloud**
- Two modes: the Cloud API (`ISteamRemoteStorage`), where new files sync to all platforms by default and `SetSyncPlatforms` changes that, and Auto-Cloud, which uses file Root Paths, needs no code, and syncs at launch and exit. Byte and file-count quotas are set per user per game by the developer. A single write is capped at 100MB. Files of 256MB or more may perform poorly. Auto-Cloud roots include `SteamCloudDocuments` (all OSes), `MacAppSupport`, `LinuxXdgDataHome` and `WinAppDataRoaming`. Cross-platform Auto-Cloud requires Root Overrides. A demo can share cloud storage with the main app through "Shared cloud APP ID". **[fetched]** — [Steamworks: Steam Cloud](https://partner.steamgames.com/doc/features/cloud)

**Workshop / UGC**
- Uses ISteamUGC. A "Ready-to-use" workshop has no curation, and authors can update items at any time. A "Curated" workshop requires developer approval and community voting. Ready-to-use needs no item server. A game appears on the Workshop front page only after at least one item is published. Users with temporary licenses (Family Sharing, free weekends) cannot upload. Size quotas are not given on the overview page. Tags are covered on a separate page. **[fetched]** — [Steamworks: Workshop](https://partner.steamgames.com/doc/features/workshop)

**Timeline / Game Recording (2024 feature)**
- Steam Timeline is part of the Game Recording framework. Recording works with no API calls; the optional ISteamTimeline calls add value: `SetTimelineTooltip` for the state description, `AddInstantaneousTimelineEvent` for markers with an icon and clip priority, and game-phase tags. Icons can use the built-in `steam_` set or custom icons uploaded in Steamworks. — [Steamworks: Timeline](https://partner.steamgames.com/doc/features/timeline); [ISteamTimeline](https://partner.steamgames.com/doc/api/ISteamTimeline); [GodotSteam timeline](https://godotsteam.com/classes/timeline/)

**Other features (not re-fetched)**
- Achievements and stats (ISteamUserStats); Rich Presence (ISteamFriends::SetRichPresence, with localization tokens) that shows "Exploring Seahorse Valley at 1e-42" to friends; Steam Input (action sets, with glyphs supplied by the API); Remote Play Together works automatically for local-multiplayer games; and the Overlay, which is required for the on-screen keyboard and purchases. **[not re-fetched]** — [Achievements](https://partner.steamgames.com/doc/features/achievements); [Rich Presence](https://partner.steamgames.com/doc/features/enhancedrichpresence); [Steam Input](https://partner.steamgames.com/doc/features/steam_controller); [Remote Play](https://partner.steamgames.com/doc/features/remoteplay)
- Trading cards are not available at launch. Valve has gated eligibility behind a "confidence" or sales threshold since 2017. **[not re-fetched]** — [Steamworks: Trading Cards](https://partner.steamgames.com/doc/marketplace/tradingcards)

### Inferences
- **Daily challenge:** create one leaderboard per day, named for example `daily_2026-10-14`, with FindOrCreateLeaderboard (requires that boards can be created at runtime; not verified). Use KeepBest. Store the seed and version in the 64-int details. 10,000 boards covers about 27 years of dailies, but you should also prune or reuse them.
- **Canyon racing with ghosts:** create one leaderboard per track with an ascending sort and the Milliseconds display type. The flown path (a compact keyframe stream) goes into a Cloud file, is shared with FileShare, and is attached with AttachLeaderboardUGC. This is exactly the pattern Valve documents.
- **Anti-cheat:** with client writes, any score can be forged. Options: (a) accept client-trust leaderboards with Friends-first display. That is cheap and common for indie games. (b) Set boards to Trusted and run a small server that checks the ghost replay deterministically, then calls SetLeaderboardScore. Fractal iteration is deterministic only if the precision arithmetic is reproducible across CPUs and GPUs, so do server validation on CPU with the same arbitrary-precision library.
- **Discovery atlas:** Workshop can host "locations, flights, palettes" as shareable items (coordinates plus parameters, with a thumbnail as the preview image). But "first discoverer of region X" needs an atomic, globally consistent claim with timestamps and dedup by region. Neither Workshop nor leaderboards provide that safely, and FindOrCreateLeaderboard per region would quickly hit the 10k cap. **A small backend is required**, for example a serverless function plus a database, authenticated with Steam session tickets (ISteamUser::GetAuthTicketForWebApi, validated server-side through the Web API). This backend can also handle trusted leaderboard writes.
- **Timeline:** call AddInstantaneousTimelineEvent on "new depth record", "discovery claimed" and "race finish". That gives players cheap shareable clips, which helps marketing.

### Gaps
- Documented size limits for UGC attached to leaderboards and per-item Workshop quotas were not found.
- I did not verify whether leaderboards can be created at runtime in unlimited numbers (only the 10k cap is documented), or how Valve views generating one board per day.
- No Valve guidance was found on cheating policy for leaderboards beyond the Trusted writes setting.

---

## 4. Platform specifics (Deck Verified, Proton vs native, macOS, OS share, GPU APIs, specs)

### Takeaway
In September 2026, Windows had 95%, Linux about 3% and macOS about 2% of Steam survey users. A Mac build is a small slice of Steam sales. Deck Verified mostly comes down to defaults: full controller support, glyphs that match the input device, text at least 9px at 1280×800, 30fps, and no launcher. Valve tests the native Linux build first and falls back to Proton, then reports whichever result is better.

### Cited Findings
- Steam Hardware & Software Survey, September 2026: Windows 95.03% (+1.08), Linux 3.05% (−0.85), macOS 1.92% (−0.22). The Linux drop coincided with a rise in Simplified Chinese usage. "SteamOS Holo" was 17.37%, presumably as a share of Linux users. This is secondary reporting; Valve's page was not fetched. — [GamingOnLinux, Oct 2026](https://www.gamingonlinux.com/2026/10/steam-survey-for-september-2026-shows-a-drop-in-linux-with-simplified-chinese-rising/)
- Linux reached a record 5.33% in March 2026, then fell to 4.52% in April, 3.99% in May and 3.69% in June. July was 4.01%, with macOS at 2.32%. — [KitGuru](https://www.kitguru.net/gaming/joao-silva/steamos-propels-linux-to-record-5-33-share-in-latest-steam-survey/); [GamingOnLinux May 2026](https://www.gamingonlinux.com/2026/06/steam-survey-for-may-2026-is-out-linux-down-at-3-99-percent-but-still-above-macos/comment_id=295695)
- **Deck Verified criteria:**
  - Input: default controller configuration with no in-game changes; glyphs that match the active device; the Steamworks on-screen keyboard or controller-friendly text entry.
  - Display: 1280×800 or 1280×720; smallest text ≥9px at 1280×800, with 12px and configurable size recommended.
  - Performance: 30fps at 800p by default.
  - No "unsupported OS/GPU" warnings, and launchers must be controller-navigable (Valve discourages them).
  - Proton-blocking middleware such as some anti-cheat or codecs leads to Unsupported.
  - Ratings: Verified, Playable, Unsupported, Unknown. Deck and Steam Machine ratings are independent.

  **[fetched]** — [Steamworks: Steam Deck Compatibility](https://partner.steamgames.com/doc/steamdeck/compat)
- Review process: Valve tests the native Linux build by default. If it fails, Valve tests the Windows build under Proton and submits the more favorable result. Developers can request a review or re-review from Technical Tools after passing the standard build review. Results auto-publish after about one week. **[fetched]** — [Steam Deck Compatibility](https://partner.steamgames.com/doc/steamdeck/compat)
- macOS on Steam: Valve has required new macOS apps to be **64-bit and notarized** since 2019. In Steamworks you tick "64 Bit Binaries Included" and "App Bundles Are Notarized". The entitlement `com.apple.security.cs.disable-library-validation` is needed so Steamworks and overlay dylibs can load. Steam is "not currently compatible with the com.apple.security.app-sandbox entitlement". The docs' stated minimum is still Intel Macs on 10.11, which is stale. **[search snippet of official page]** — [Steamworks: Platforms](https://partner.steamgames.com/doc/store/application/platforms)
- No official Steam rule requiring arm64 or universal binaries was found. A community report says the Steam client stub became universal. Third-party libraries that are Intel-only break universal builds. — [Steam Community](https://steamcommunity.com/discussions/forum/2/686357829243940409); [Edenwaith](https://www.edenwaith.com/blog/index.php?p=161)

### Inferences
- Because Steam's Mac build cannot use the App Sandbox, the **Mac App Store binary and the Steam binary must be different builds**: the MAS build is sandboxed with no Steamworks, and the Steam build is non-sandboxed, notarized and Developer ID signed. Ship the Steam Mac depot as a universal2 or arm64 binary; Apple Silicon is effectively the whole current Mac market (not separately sourced here).
- A native Linux build is optional for Deck: Valve will fall back to Proton. If almondbread's renderer uses wgpu or Vulkan, a native Linux build is cheap and avoids Proton translation issues. If it is DX12-only, Proton through vkd3d is usually fine. Either way, test on a Deck: deep-zoom compute load at 800p and 30fps is the risk.
- GPU API: Vulkan on Windows and Linux plus Metal on macOS (for example through wgpu or MoltenVK) covers all three platforms. DX12 adds nothing for this app. Minimum specs should state a GPU feature level and the CPU path for arbitrary-precision math. These points are inferences; GPU-API share data was not researched.
- Deck UX: fractal navigation maps naturally to sticks and triggers (pan and zoom). Use Steam Input action sets, not raw XInput, to get correct glyphs on Deck. Coordinate entry needs the on-screen keyboard.

### Gaps
- Valve's own survey page was not fetched, and no Apple Silicon versus Intel split for Steam Mac users was found.
- No current (2025–2026) Steamworks macOS doc confirming arm64 guidance was found.
- No source was found on minimum-spec conventions or DX12/Vulkan share among Steam users.

---

## 5. Cross-store and legal (Mac App Store + Steam, IARC/age ratings, privacy)

### Takeaway
Steam and the Mac App Store need separate builds and separate SKUs. Steam sandboxing is incompatible, so Steamworks features such as achievements, cloud and leaderboards do not exist on the MAS build. Cross-progression would require your own backend or a portable save format. I did not research the details of age rating and privacy in this session.

### Cited Findings
- Steam does not support the App Sandbox entitlement that the Mac App Store requires. — [Steamworks: Platforms](https://partner.steamgames.com/doc/store/application/platforms)
- Steam Cloud quotas and sync behaviour are Steam-only. Cross-platform sync between Steam OSes is supported through the Cloud API, or through Auto-Cloud Root Overrides. — [Steam Cloud](https://partner.steamgames.com/doc/features/cloud)
- Steam's store setup includes a content survey and an optional IARC rating questionnaire, which produces ratings for regions such as Germany (USK) and Australia. **[not re-fetched]** — [Steamworks: Content Survey / Ratings](https://partner.steamgames.com/doc/store/ratings)

### Inferences
- Pricing parity is not enforced across Steam and the MAS by any source found, but Apple takes a 15% cut through its Small Business Program and Steam takes 30%. Expect pressure to price the same and absorb the difference.
- Cross-progression between the MAS and Steam requires your own account system or an export/import of a portable save file (bookmarks and flights as JSON or URLs). The deep-zoom location format is a natural "share code" that works across stores without a backend.
- Any backend for the atlas or trusted leaderboards will process Steam IDs and possibly IP addresses, which brings in GDPR. You will need a privacy policy linked from the store page and a stated data-retention policy. This is a general EU-law inference and was not researched here.

### Gaps
- IARC on Steam (current process and whether it is mandatory for Germany), Steam's privacy policy requirements for games with online features, and the EU Digital Services Act and consumer-law implications for a solo seller were not researched in this session.
- No source was found on Valve's position regarding price parity with other stores.
