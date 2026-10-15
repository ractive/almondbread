---
title: "Cosmic size scale: sources for galaxies, cosmology, life, particles and big numbers"
type: source
authors: ["NASA", "Planck Collaboration", "Caltech IPAC", "R. Milo", "R. Phillips", "ZEUS Collaboration", "CERN", "A. J. Buras", "M. M. Vopson", "C. E. Shannon", "E. Kasner", "J. R. Newman"]
year: 2026
raw: "web"
status: digested
tags: [cosmic-scale, overlay, reference-values]
---

# Cosmic size scale: other references

A collection page: every value the cosmic size scale overlay needs that is not on
[[sources/nasa-planetary-fact-sheets]], [[sources/iau-astronomical-units]] or
[[sources/codata-2022-constants]]. Read 2026-10-15; nothing saved under `raw/`.
"Typical" sizes are order-of-magnitude markers; give them with "~" in the overlay.

## Stars, galaxies, the universe

| Object | Figure as quoted | SI (1 ly = 9.4607e15 m) | Source | Reliability |
| --- | --- | --- | --- | --- |
| Proxima Centauri distance | "red dwarf star Proxima Centauri is 4.24 light-years away" | 4.01e16 m | NASA, Sun facts, <https://science.nasa.gov/sun/facts/> | High |
| Alpha Centauri A/B distance | "Alpha Centauri A and B … are 4.37 light-years away." | 4.13e16 m | same | High |
| Milky Way diameter | "The disk of the Milky Way galaxy is about 100,000 light years in diameter" | ~9.5e20 m | NASA Imagine the Universe, <https://imagine.gsfc.nasa.gov/ask_astro/galaxies.html>; also "about 100,000 light-years across", NASA 2025, <https://www.nasa.gov/science-research/astrophysics/how-big-is-space-we-asked-a-nasa-expert-episode-61/> | Good as a round figure; the stellar disk edge is not sharp and some studies give larger values |
| Andromeda (M31) distance | "at a distance of approximately 2.5 million light-years" | ~2.37e22 m | NASA Hubble, 16 Jan 2025, <https://science.nasa.gov/missions/hubble/nasas-hubble-traces-hidden-history-of-andromeda-galaxy/> | Good (measured values 2.4–2.6 Mly). The ESA/Hubble heic2501a info box says 2 Mly, which contradicts its own text |
| Observable universe diameter | "the observable universe is about 92 billion light-years across" | ~8.7e26 m | NASA 2025 (link above); Caltech IPAC Cool Cosmos: "the size of the observable universe is actually 92 billion light years", <https://coolcosmos.ipac.caltech.edu/ask/237-How-big-is-the-Universe-> | Medium. NASA and Caltech say **92** Gly. The common "93 billion ly" (comoving radius ~46.5 Gly) appears only in secondary sources (Wikipedia) — (unverified) as a primary figure. Use ~9e26 m or "~92–93 billion ly" |
| Age of the universe | `13.787 ± 0.020` Gyr | 4.351e17 s | Planck 2018 results VI, Table 2, last column (TT,TE,EE+lowE+lensing+BAO), A&A 641, A6 (2020), <https://arxiv.org/abs/1807.06209> | High (base ΛCDM; the CMB-only column gives 13.797 ± 0.023) |

## Earth and life

| Object | Figure as quoted | SI | Source | Reliability |
| --- | --- | --- | --- | --- |
| Mount Everest | 8,848.86 m (China–Nepal joint announcement, 8 Dec 2020) | 8848.86 m | Himalayan Times, <https://thehimalayantimes.com/nepal/everest-2-0-worlds-tallest-peaks-revised-height-revealed-stands-at-8848-86m/> | Value certain; source is news reporting of the official announcement (no government page fetched) |
| Adult human height | ~1.7 m | 1.7 m | No single citable "typical" figure found. NCD-RisC 2016 (eLife, <https://pmc.ncbi.nlm.nih.gov/articles/PMC4961475/>) gives country means from 140 cm to over 182.5 cm | (unverified) as a single number; a round marker |
| Human hair diameter | "according to Kaswell 15µm - 110µm: according to Randebrook 40µm - 120µm" | ~5e-5 to 1e-4 m | BioNumbers 116402 (Robbins, *Chemical and physical behavior of human hair*, 2002), <https://bionumbers.hms.harvard.edu/bionumber.aspx?id=116402> | Medium; wide natural range. The often-quoted "17–181 µm" traces to a cosmetics description, avoid |
| Red blood cell diameter | "A typical RBC measures approximately 6.2 to 8.2 µm in diameter" | ~7.5e-6 m | StatPearls, Poikilocytosis, NCBI Bookshelf, <https://www.ncbi.nlm.nih.gov/sites/books/NBK562141/> | High for the range; ~7.5 µm is a fair midpoint |
| E. coli length | "a diameter of about ≈1µm, a length of ≈2µm" | 2e-6 m | Milo & Phillips, *Cell Biology by the Numbers*, <https://book.bionumbers.org/how-big-is-an-e-coli-cell-and-what-is-its-mass/> | High as a rule of thumb (measured lengths 1.8–2.9 µm depending on method and growth) |
| Typical virus | "a characteristic length scale of roughly 100 nanometers" | 1e-7 m | Milo & Phillips, <https://book.bionumbers.org/how-big-are-viruses/> | High as an order of magnitude. Influenza: "80–120 nm" per reviews on PMC (search snippets only; not opened) |
| DNA double helix width | "its diameter is about 20 Å (2 nm)" | 2e-9 m | Snippet of a PMC review (Structural DNA Nanotechnology: An Overview, <https://pmc.ncbi.nlm.nih.gov/articles/PMC3478330/>); the page itself was behind a captcha | Value standard (B-form 1.9–2.4 nm by conditions); quote seen only via search snippet, so (unverified) wording |

## Nuclei and particles

| Object | Figure as quoted | SI | Source | Reliability |
| --- | --- | --- | --- | --- |
| Nuclear radius | "R=r_{0} A^{1 / 3}" with r0 "about 1.2 fm" | Gold (A=197): 7.0e-15 m; lead-208: 7.1e-15 m | LibreTexts, Modeling the Nucleus, <https://phys.libretexts.org/Courses/Coalinga_College/Physical_Science_for_Educators_Volume_2/10%3A_Nuclear_Physics/10.04%3A_Modeling_the_Nucleus> | Formula standard; the page's worked gold example is wrong (uses 179 for 197, gets 6.76 fm). r0 ranges ~1.2–1.3 fm. HyperPhysics shows the formula only as an image |
| Proton radius | see [[sources/codata-2022-constants]] (0.84075 fm) | | | |
| Quark radius upper limit | "The resulting 95% C.L. upper limit on the effective quark radius is 0.43·10^-16 cm." | < 4.3e-19 m | ZEUS Collaboration, Phys. Lett. B 757 (2016) 468, arXiv:1604.01280, <https://ar5iv.arxiv.org/html/1604.01280> | High; model-dependent (form-factor model) |
| LHC collision energy | "the record collision energy of 13.6 trillion electronvolts (TeV)" | 13.6e12 eV; λ = hc/E ≈ 9.1e-20 m | CERN Run 3 media kit, <https://home.cern/press/media-kits/run-3/> | High |
| Smallest scale probed directly | "The Large Hadron Collider will directly probe distance scales as short as 10^{-19}m" | ~1e-19 m | Buras, Buttazzo, Girrbach-Noe, Knegjens, JHEP 11 (2014) 121, arXiv:1408.0728, <https://arxiv.org/abs/1408.0728> | Medium: a physicist's round statement, not a measured record. Indirect flavour probes reach ~1e-21 m (Buras, arXiv:2403.02387). Say "~1e-19 m, the shortest distance probed directly at colliders" |

No electron-size limit was looked up; use the quark limit instead.

## Big numbers

| Quantity | Figure as quoted | Source | Reliability |
| --- | --- | --- | --- |
| Atoms in the observable universe | ~1e80. Vopson derives "1.93 × 10^80 baryons" and "Ntot = 4 × 10^80 particles" | Vopson, *AIP Advances* 11, 105317 (2021), arXiv:2112.04473, <https://arxiv.org/abs/2112.04473> | Order of magnitude only; estimates span 1e78–1e82. No agency page found; the "~1e80 atoms" phrasing is secondary (Wikipedia, *Large numbers*) |
| Shannon number | "even at this figure there will be 10^120 variations to be calculated from the initial position" | Shannon, "Programming a Computer for Playing Chess", Phil. Mag. Ser. 7, 41 (314), 256–275, March 1950. Text transcription: <https://www.pi.infn.it/~carosi/chess/shannon.txt> | High as a citation (it is a rough lower-bound estimate of the game tree, from ~10^3 per move pair over 40 moves) |
| Googol | "A googol is a large number equal to 10^(10^2)=10^(100)" | Wolfram MathWorld, <https://mathworld.wolfram.com/Googol.html>; coined 1938 by Milton Sirotta, nephew of Edward Kasner; MathWorld cites Kasner & Newman, *Mathematics and the Imagination*, pp. 20–27 (1989 reprint) | Definition exact. First edition 1940 (Simon & Schuster) is (unverified) here; MathWorld cites only the reprint |

## Times

Epochs after the Big Bang are on [[sources/cosmic-history-timeline]]. 1 yr = 3.15576e7 s.

| Quantity | Figure as quoted | SI | Source | Reliability |
| --- | --- | --- | --- | --- |
| Age of the Sun | "The Sun formed about 4.6 billion years ago in a giant, spinning cloud of gas and dust." | 1.45e17 s | NASA, Sun facts, <https://science.nasa.gov/sun/facts/> | High. The same page's intro says "a 4.5 billion-year-old yellow dwarf"; use 4.6 (also the PDG pre-solar nebula age) |
| Homo sapiens | "Dated to 300 thousand years ago these early Homo sapiens already have a modern-looking face" | 9.5e12 s | Hublin et al., "New fossils from Jebel Irhoud (Morocco) and the pan-African origin of Homo sapiens", *Nature* 546, 289 (2017), doi:10.1038/nature22336; quote from the MPI-EVA release, <https://www.eva.mpg.de/homo-sapiens.html>. Site dating: Richter et al., *Nature* (2017), doi:10.1038/nature22335 | High as "~300 000 years" (oldest known fossils; the dating has an error of several tens of kyr). Volume and page were not shown on the fetched pages, (unverified) here |
| Shortest time measured | "about 247 zeptoseconds for the average bond length of the molecule" (photon crossing an H2 molecule) | 2.47e-19 s | Grundmann et al., "Zeptosecond birth time delay in molecular photoionization", *Science* 370, 339 (2020), doi:10.1126/science.abb9318; DESY press release, <https://photon-science.desy.de/news__events/news__highlights/archive/archive_of_2020/zeptoseconds_new_world_record_in_short_time_measurement_at_petra_iii/PIF_eng.html> | High as of 2020 ("new world record" per DESY). A delay measurement, not a pulse length |

## Contradictions

- Sun age: 4.6 Gyr and 4.5 Gyr on the same NASA page.
- Observable universe: 92 Gly (NASA, Caltech) against the commonly quoted 93 Gly
  (secondary). Both round the comoving diameter; not worth a question page.
- Andromeda: 2.5 Mly (NASA, ESA/Hubble text) against 2 Mly (ESA/Hubble image box).
