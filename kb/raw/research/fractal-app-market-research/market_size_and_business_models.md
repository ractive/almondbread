# Market size signals and business models for fractal / generative-visual apps (as of Oct 2026)

Scope: input for pricing and business model of "almondbread" (premium deep-zoom Mandelbrot explorer; macOS/iOS/iPadOS first, Windows/Linux later; indie developer).
Method: web search + fetch, 2026-10-14. Several App Store pages returned HTTP 429, so some current App Store prices could not be verified directly. Every third-party download/revenue number below is a model estimate, not developer-reported.

## 1. Downloads and revenue of fractal apps and adjacent apps

### Takeaway
No public, developer-reported revenue exists for any fractal app. The best-known iOS fractal app (Frax) has an estimated 250k+ lifetime downloads and only a few hundred ratings. That points to a small niche: lifetime gross probably in the low hundreds of thousands of USD, not millions. The adjacent "ambient visuals on your desktop" category, led by Wallpaper Engine on Steam, is orders of magnitude larger.

### Cited Findings
- Frax (Iter9 LLC; Ben Weiss, Kai Krause, Tom Beddard) launched Oct 2013 for iPhone/iPad. It is a free app with a Pro in-app purchase that unlocks about 100 parameters and renders up to 50 MP — [CGPress](https://cgpress.org/archives/frax___fractals_explorer_app_released.html); [Frax about](https://www.fr.ax/about)
- Frax launch pricing: the Pro upgrade was $6.99 (or $4.99 for Frax HD owners). That is 2013 data; the current price was not verified — [CG Channel 2013](https://www.cgchannel.com/2013/10/kais-power-tools-veterans-release-frax/)
- MWM Intelligence (a third-party estimate tracking "15M+ apps", data as of Aug 17 2026) puts Frax at **250k+ downloads**, 4.5 stars from 338 ratings, ranked "outside top 30" in US Free Graphics & Design (iPhone). It lists IAPs for Pro, 200 MP renders and Frax Cloud. No revenue estimate is given — [mwm.ai Frax](https://mwm.ai/apps/fra/568827824)
- A third-party aggregator claims Frax has had no release in over 18 months. This is unverified — [marlvel.ai](https://marlvel.ai/apps/fra)
- No Frax subscription and no subscription complaints were found. Pro is a one-time IAP — [appfollow Frax](https://apps.appfollow.io/ios/frax/568827824?country=sv)
- Fractal Architect 5 (Mac, flame fractals) costs $19.99 with IAPs: GPU Rendering $54.99 and Animation Sequencer $24.99. Fractal Architect X is free, with Silver (unlimited image size) and Gold (GPU rendering) subscriptions, monthly or yearly; prices were not retrieved — [App Store FA5](https://apps.apple.com/us/app/id1331123802); [App Store FA X](https://apps.apple.com/np/app/fractal-architect-x/id1331145352?mt=12)
- Fractal Architect iOS is listed at $9.99 with no IAPs, per data last updated Apr 2023 — [AppPricingLab](https://www.apppricinglab.com/app/apple/1445109024)
- Ultra Fractal 6 (Windows/Mac desktop, direct sale): Express $29, Creative $49, Extended $99 (animation, network rendering). It offers a 30-day trial with watermarked output, discounted upgrades for earlier-version owners, discounted classroom site licenses on request, and a 30-day refund — [Ultra Fractal shop](https://ultrafractal.com/shop); [Editions](https://ultrafractal.com/editions.html); [Purchasing](https://ultrafractal.com/help/purchase/purchase.html)
- There are many small, mostly free Mandelbrot apps on the App Store: Fractally (free; iPhone/iPad/Mac; claims >10^12 zoom), Mandelbrot Metal (claims 10^16), RetinaMandelbrot (claims 10^20000), Mandel-Zoom, Ech's Fractal and Mandlebrot Infinite — [Fractally](https://apps.apple.com/fr/app/fractally/id1534380451); [Mandelbrot Metal](https://apps.apple.com/app/id6749807290); [RetinaMandelbrot](https://apps.apple.com/us/app/retinamandelbrot/id1509664268?uo=4); [Ech's Fractal](https://apps.apple.com/app/id1315403987); [mwm.ai Mandlebrot Infinite](https://mwm.ai/apps/mandlebrot-infinite/6762680008)
- Adjacent category, Wallpaper Engine (Steam, $4.99): VaporLens estimates about 50M copies and about $250M revenue, modeled from about 978k reviews (98% positive), updated May 2026. This is an estimate, not official — [VaporLens](https://vaporlens.app/app/431960/wallpaper_engine/stats/details); [Mein-MMO](https://mein-mmo.de/wallpaper-engine-4euro-steam-top-spiel-steam/)

### Inferences
- Frax has the strongest pedigree and the most press in the category, yet has only about 250k+ downloads in 13 years. A one-time Pro IAP at about $5–10, with a typical 2–5% free-to-paid rate (see section 3), would imply lifetime IAP gross on the order of **$25k–$125k** (250k × 2–5% × ~$7–10). This is an inference with wide error bars, not a reported figure.
- The competitive set is either free/cheap hobby apps or aging premium apps (Frax possibly unmaintained, Fractal Architect is a 2010s-era product, and Ultra Fractal is Windows-centric and old-fashioned in UX). That leaves room for a modern premium native app, but the demand ceiling is low.
- The deep-zoom niche specifically (perturbation, arbitrary precision) is served on desktop mostly by free tools such as Kalles Fraktaler and Fractal Zoomer, which were not researched here. Free tools anchor desktop price expectations low.
- Wallpaper Engine shows that "beautiful generative visuals as ambient desktop/screensaver" can reach mass market at a low one-time price on Steam. Exporting Mandelbrot zooms as wallpapers or screensavers is a more scalable hook than "fractal explorer" as such.

### Gaps
- No Sensor Tower or appfigures public snippets with revenue for Frax, Fractal Architect or any Mandelbrot app were found.
- No developer interview or postmortem with Frax or Fractal Architect revenue was found.
- Current Frax Pro price and Fractal Architect X subscription prices are unverified (App Store returned 429).
- Revenue for music visualizers, "Endless"-type visualizers and Shadertoy-style apps was not researched for lack of tool budget.

## 2. Pricing norms in the category

### Takeaway
One-time purchases dominate. Mobile unlocks run about $5–10, Mac apps about $20 plus feature IAPs, and desktop pro tools $29–$99 in tiers. Subscriptions exist (Fractal Architect X) but are the exception. Tiered feature unlocks (GPU rendering, animation, large renders) are the established IAP pattern.

### Cited Findings
- Frax: free download with one-time Pro IAP (historically $6.99) plus separate high-res render and cloud IAPs — [CG Channel](https://www.cgchannel.com/2013/10/kais-power-tools-veterans-release-frax/); [mwm.ai](https://mwm.ai/apps/fra/568827824)
- Fractal Architect 5: $19.99 paid plus $54.99 and $24.99 feature IAPs. Fractal Architect X: free plus Silver/Gold subscriptions — [FA5](https://apps.apple.com/us/app/id1331123802); [FA X](https://apps.apple.com/cn/app/id1331145352)
- Ultra Fractal: three one-time editions at $29, $49 and $99, upgrade pricing for major versions, and a trial with watermark — [Ultra Fractal shop](https://ultrafractal.com/shop)
- Universal Purchase (one purchase covering iOS, iPadOS and macOS, with IAPs shareable across platforms) has been available for Mac apps since 2020. It requires the same bundle ID; 1Blocker had to re-release its Mac app to adopt it — [MacStories](https://www.macstories.net/news/universal-purchases-for-mac-apps-are-now-available-to-developers/); [9to5Mac](https://9to5mac.com/2020/02/05/apple-adds-ability-for-developers-to-sell-mac-and-ios-apps-as-a-single-purchase/); [1Blocker](https://news.1blocker.com/meet-universal-purchase-family-sharing/)
- Pixelmator Photo's 2022 move to subscription illustrates the trade-off. Upfront pricing forced reliance on major-update buzz, and only new users generated revenue — [PetaPixel](https://petapixel.com/2022/08/19/pixelmator-photo-is-shifting-to-subscription-pricing/)
- Indie developers explicitly market a one-time price as the "honest alternative" to subscriptions, but give no revenue data — [DEV Community](https://dev.to/rafay_korai_cb5d79a6a13fa/i-charge-29-once-for-software-that-competitors-charge-144year-for-heres-why-44hf)
- The general commentary on subscription fatigue in the App Store points the same way — [iDownloadBlog](https://www.idownloadblog.com/?p=781335)

### Inferences
- A defensible price anchor for almondbread is about **$10–20 one-time Universal Purchase** (iPhone/iPad/Mac), possibly with a higher "Pro/Studio" tier (about $30–50) for features like 8K+/print-size renders, video zoom export and batch/animation. That brackets Frax, FA5 and Ultra Fractal Express/Creative.
- A pure subscription would be an outlier in this niche. An exploration tool with no server costs offers little ongoing value to justify one, and subscription fatigue is a known complaint. If recurring revenue is wanted, paid major-version upgrades (the Ultra Fractal model) fit category norms better.
- Palette or formula "packs" as IAP have precedent only weakly (Frax sells render-size and cloud IAPs, not content packs). Feature-gated IAPs (GPU, animation, resolution) are the proven pattern.

### Gaps
- No review mining on subscription sentiment specific to Fractal Architect X was possible (App Store blocked).
- No evidence was found on Family Sharing uptake effects for niche apps.

## 3. Business models that work for niche creative/education apps (2024–2026); conversion benchmarks

### Takeaway
RevenueCat's 2026 data says hard paywalls convert about 5× better than freemium (10.7% vs 2.1% trial-to-paid at day 35) and earn about 8× the revenue per install. Those are subscription-app medians, though. For one-time unlocks the only defensible public benchmark remains a roughly 2–5% free-to-paid rate. Setapp, Steam and direct sales are viable secondary channels.

### Cited Findings
- RevenueCat State of Subscription Apps 2026 covers 115k+ apps and $16B+ revenue. Its paywall and revenue findings:
  - Median day-35 conversion: hard paywall 10.7% (12.1% in 2025) vs freemium 2.1%.
  - Revenue per install at day 60: hard paywall $3.09 vs freemium $0.38.
  - Yearly-subscriber 1-year retention is about equal: freemium 28%, hard paywall 27%.
  - Source: [RevenueCat benchmarks blog](https://www.revenuecat.com/blog/growth/subscription-app-trends-benchmarks-2026); [Report](https://www.revenuecat.com/state-of-subscription-apps)
- RevenueCat 2026 on trials and churn:
  - Trial-to-paid: 42.5% for 17–32 day trials vs 25.5% for trials under 4 days.
  - About 72% of annual subscribers cancel in year 1 (up from about 56%).
  - The revenue gap between the top 5% and bottom 25% of apps is 400×.
  - Source: [RevenueCat blog](https://www.revenuecat.com/blog/growth/subscription-app-trends-benchmarks-2026)
- A secondary write-up of the RevenueCat 2026 report gives regional median conversion of 2.6% in North America and 2.0% in Western Europe, with a North America 90th percentile of 10.4% — [ppc.land](https://ppc.land/ai-apps-earn-41-more-per-user-but-churn-30-faster-revenuecat-finds/)
- RevenueCat has category cuts for Education, Productivity, Utilities and others. The details sit behind the report download — [RevenueCat Education 2026](https://www.revenuecat.com/state-of-subscription-apps-2026-education)
- Free plus one-time unlock case: Super Mario Run had about a 3% payer rate on about 90M downloads (Newzoo estimate, 2017) — [9to5Mac](https://9to5mac.com/2017/01/03/super-mario-run-revenue-conversion/)
- Generic freemium free-to-paid conversion is commonly cited at 2–5%. These are secondary, low-rigor sources — [The Swift Kit](https://theswiftk.it.com/blog/how-to-monetize-ios-app-indie-developer); [AppDeveloperMagazine](https://appdevelopermagazine.com/freemium-versus-paid-app-models/)
- Setapp economics:
  - Membership splits 70% of each subscriber's fee across the apps that user opened in the period. One launch counts, frequency is ignored, and price-tier multipliers apply.
  - A +20% partner fee applies for referred users (up to 90%).
  - Single-App Distribution pays 85%.
  - Source: [Setapp docs: distributing revenue](https://docs.setapp.com/docs/distributing-revenue); [Setapp developers](https://setapp.com/developers)
- Setapp, a vendor with an interest, says subscription works best for apps with long-term value and one-time purchase remains effective for specialized utilities — [Setapp](https://setapp.com/app-reviews/app-monetisation-strategies)
- Ultra Fractal offers discounted classroom/site licenses on request, which is the established education channel for desktop fractal software — [Ultra Fractal purchasing](https://ultrafractal.com/help/purchase/purchase.html)

### Inferences
- RevenueCat's "hard paywall wins" applies to subscriptions with trials. The closest analog for almondbread is **paid up front, or free download with a near-immediate one-time unlock (a "soft-hard" paywall)**. A generous free tier risks the 2% freemium regime.
- Recommended evidence-consistent model: free download with limited but delightful exploration (capped zoom depth or render resolution, watermark on export), plus a one-time Universal Purchase unlock. Paid major upgrades every 2–3 years would be optional. A trial-style "full features for N days" could borrow RevenueCat's finding that longer trials convert better, but that evidence is from subscriptions.
- Setapp fits a Mac app that users open occasionally only weakly. Payout depends on the active-user share across many apps, so it is incremental exposure, not a primary revenue channel. Single-App Distribution at 85% is equivalent to the Small Business Program rate.
- Education (Apple School Manager volume purchasing) is plausible for a math-visualization app but unquantified. Treat it as an upside channel.

### Gaps
- No public benchmark exists for one-time-unlock conversion in creative or education niches on iOS/Mac in 2024–2026.
- Category-level RevenueCat data (Photo & Video, Education) was not retrievable without the gated PDF.
- Apple School Manager / volume purchase terms (e.g. bulk discount thresholds) were not verified this session; check Apple's developer documentation.
- No indie postmortem with revenue for a niche creative Mac/iOS app from 2024–2026 was found.

## 4. Distribution levers (editorial, design awards, keywords, Steam)

### Takeaway
Organic search demand for "fractal" or "mandelbrot" is unknown publicly and likely low. Visual distinctiveness is the lever. Apple Design Award categories such as "Delight and Fun" and "Visuals and Graphics" reward exactly this kind of app, and 2026 finalists include small indie visual toys. On Steam, expect first-week sales of roughly 10–20% of launch wishlists, lower for higher-priced titles.

### Cited Findings
- 2026 Apple Design Award finalists:
  - Delight and Fun (apps): Blippo+, Metaballs, Grug.
  - Visuals and Graphics (apps): Tide Guide, Caradise, (Not Boring) Camera.
  - Each category had 3 app and 3 game finalists; winners were announced at WWDC on June 8, 2026 (not verified here).
  - Source: [9to5Mac](https://9to5mac.com/2026/05/18/apple-unveils-30-apple-design-award-app-finalists/); [MacRumors](https://www.macrumors.com/2026/05/18/apple-design-award-finalists-2026); [iGeeksBlog](https://www.igeeksblog.com/apple-design-awards-2026-finalists/)
- App Store keyword demand is visible only as Apple Search Ads "Search Popularity" (5–100, nonlinear/exponential). Raw counts are not disclosed — [trysonar](https://trysonar.app/blog/apple-search-popularity); [AppFollow](https://support.appfollow.io/hc/en-us/articles/360020832897)
- Steam wishlist conversion (GameDiscoverCo):
  - Median first-week sales were about 0.20× launch wishlists in 2021, 0.17× for games with 10k+ wishlists, and 10.5% for Aug–Oct 2024 launches with 5k+ wishlists.
  - For Sept 2024–Sept 2025, the median was 0.15× for games with 25k+ wishlists and 0.10× for games priced above $10.
  - Cheaper games convert somewhat better.
  - Source: [GameDiscoverCo](https://newsletter.gamediscover.co/p/steam-the-new-wishlists-to-first); [GameDev Reports](https://gamedevreports.substack.com/p/gamediscoverco-the-state-of-steam); [Automaton](https://automaton-media.com/en/news/steam-wishlist-to-sales-study-shows-how-genre-release-timing-and-reviews-affect-success-nsfw-games-show-unusually-high-conversion-rate/)
- Steam supports non-game software at scale. Wallpaper Engine has about 978k reviews, a modeled estimate of about 50M copies, and a $4.99 price — [VaporLens](https://vaporlens.app/app/431960/wallpaper_engine/stats/details)
- Frax got launch press in Macworld because of its team's KPT pedigree — [Macworld](https://www.macworld.com/article/222042/introducing-frax-a-compelling-new-ios-fractal-app.html)

### Inferences
- "Metaballs" as a 2026 Delight and Fun finalist suggests Apple editorial values small, polished generative-visual toys. A Metal-native, deep-zoom Mandelbrot app that adopts current Apple design idioms is an ADA/editorial candidate. Featuring is not plannable but is the single biggest potential lever.
- With little keyword demand, the shareability of output (zoom videos, wallpapers, social clips) drives top-of-funnel more than ASO does.
- Steam benchmarks come from games. A Windows/Linux almondbread on Steam would need a wishlist base (e.g. 5–10k) to make a meaningful launch. At 0.1–0.2×, 10k wishlists ≈ 1–2k first-week sales.

### Gaps
- No actual Search Popularity scores for "fractal" or "mandelbrot" were found. Check them in Apple Search Ads or ASO tools.
- The ADA 2026 winners and whether Metaballs won were not confirmed.
- No Steam data for fractal or math-visualization tools specifically was found.

## 5. Costs and constraints (commissions, EU DMA, direct Mac sales)

### Takeaway
The realistic net is about 85% via the App Store Small Business Program (15%) and about 90–95% via Paddle for direct Mac sales before FX and refund leakage. New EU terms took effect Oct 1, 2026. They replace the per-install Core Technology Fee with a 5% Core Technology Commission for distribution outside the App Store, which makes EU alternative distribution economically neutral at best for a small developer already paying 15%.

### Cited Findings
- New EU terms (announced Aug 18 2026, effective Oct 1 2026):
  - App Store in-app purchases are 26% standard; 15% for the Small Business Program, Mini Apps Partner Program and Video Partner Program, and for auto-renew subscriptions after year 1.
  - Alternative payment inside an App Store app is 20% (10% for program members). Link-out purchases are 15% (10% for program members).
  - Alternative marketplaces and web distribution pay a 5% Core Technology Commission. The Core Technology Fee, initial acquisition fee and store services fee are removed.
  - Source: [MacRumors](https://macrumors.com/2026/08/18/eu-app-store-fee-change); [iClarified](https://www.iclarified.com/101814/apple-revises-eu-app-store-terms-replaces-core-technology-fee-with-5-commission); [Engadget](https://engadget.com/apps/apple-details-new-fee-structures-for-app-store-payments-in-the-eu-204253948.html)
- A single-source claim says small marketplace operators under €10M/year may get the 5% waived. This is unverified — [via search summary of EU coverage; see Phiture](https://phiture.com/blog/apple-new-app-store-terms-2026/)
- Paddle charges 5% + $0.50 per transaction as merchant of record, covering tax and VAT, checkout and subscriptions. Extra costs include FX spreads of about 1% and fees retained on refunds — [Paddle vs FastSpring](https://www.paddle.com/compare/fastspring); [Dodo Payments analysis](https://dodopayments.com/blogs/paddle-fees-explained)
- FastSpring's rate is a third-party estimate of about 5.9% + $0.95, and it is negotiated — [toolradar](https://toolradar.com/tools/fastspring/pricing); [fungies](https://fungies.io/fastspring-review-2026/)
- A 2023 analysis of Mac developers estimates the App Store at about 15% vs Paddle at about 5% — [Gigazine](https://gigazine.net/gsc_news/en/20231226-macos-app-cost)
- Setapp Single-App Distribution pays 85% to the developer — [Setapp docs](https://docs.setapp.com/docs/distributing-revenue)

### Inferences
- At a $15 price, Small Business Program net ≈ $12.75. Paddle direct ≈ 15 − 0.75 − 0.50 ≈ $13.75 before FX and refunds, so the advantage is only about $1 per sale. That is not enough to justify losing App Store discovery, Universal Purchase and Family Sharing. A direct-sale Mac build is most useful as a complement (for example a Paddle-licensed build for users avoiding the Mac App Store, education site licenses, or bundling with a later Windows/Linux build).
- Paddle's flat $0.50 hurts low price points. Below about $10, the App Store at 15% is comparable or cheaper.
- EU alternative distribution adds compliance work with negligible upside for an indie at 15%. Link-out at 10% (program members) is mildly attractive for direct-sale licenses, but adds complexity.
- Steam takes a standard 30% (well-known; not re-verified in this session), which is worse than the App Store SBP. It earns its place only through Windows/Linux reach and Steam discovery.

### Gaps
- Apple Small Business Program terms (15% for developers with under $1M in prior-year proceeds) are widely known but were not re-fetched from Apple's site this session. Verify at developer.apple.com/app-store/small-business-program/.
- Steam revenue-share tiers were not re-verified this session.
- Whether the Small Business Program 15% applies to EU link-out at 10%, combined with the 5% CTC for web-distributed Mac apps, is not applicable on macOS. The DMA terms concern iOS/iPadOS, and Mac direct distribution is not subject to Apple commission. This is general knowledge and not re-verified.
