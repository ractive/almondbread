---
title: "Cosmic history timeline: epoch times after the Big Bang (PDG, Planck, NASA, ESA, CERN)"
type: source
authors: ["Particle Data Group", "K. A. Olive", "J. A. Peacock", "B. D. Fields", "Planck Collaboration", "NASA", "ESA", "CERN", "M. D'Onofrio", "K. Rummukainen", "A. Tranberg", "A. Bazavov", "NIST"]
year: 2024
raw: "web"
status: digested
tags: [cosmic-scale, overlay, reference-values, cosmology]
---

# Cosmic history timeline

A collection page: the time after the Big Bang of each epoch the cosmic size scale
overlay marks, one number in seconds each. Read 2026-10-15; nothing saved under `raw/`.
Companion of [[sources/cosmic-scale-references]] (age of the universe, Sun age) and
[[sources/codata-2022-constants]] (Planck time).

Conventions: 1 yr = 1 Julian year = 3.15576e7 s. Before ~1 s every time is a
temperature converted with the radiation-era relation of the PDG review
"Big-Bang Cosmology" (Olive & Peacock, rev. Aug 2023, in S. Navas et al., PDG,
Phys. Rev. D 110, 030001 (2024)), eq. 22.44:

`t · T_MeV^2 = 2.4 · N(T)^(-1/2)`  (t in seconds, T in MeV)

with `N(T)` from the table under eq. 22.42 (`4N` = 43 for m_e < T < m_μ, 69 for
m_π < T < T_c, 205 just above T_c, 427 above m_top). Those times are our own
derivation (marked "derived"); they hold to a factor ~2, which is fine for a
logarithmic overlay.

Sources:

- PDG Big-Bang Cosmology: <https://pdg.lbl.gov/2024/reviews/rpp2024-rev-bbang-cosmology.pdf>
- PDG Big-Bang Nucleosynthesis (Fields, Molaro, Sarkar, rev. Aug 2023):
  <https://pdg.lbl.gov/2024/reviews/rpp2024-rev-bbang-nucleosynthesis.pdf>

## Epochs

| Epoch | Value (s) | Figure as quoted | Source | Reliability |
| --- | --- | --- | --- | --- |
| Today | 4.351e17 | `13.787 ± 0.020` Gyr | Planck 2018 results VI, <https://arxiv.org/abs/1807.06209>; see [[sources/cosmic-scale-references]] | High (base ΛCDM). PDG quotes t0 = (13.80 ± 0.02) Gyr |
| Sun and Earth form | 2.90e17 (= 13.787 − 4.6 Gyr = 9.19 Gyr) | "The Sun formed about 4.6 billion years ago" | NASA Sun facts, <https://science.nasa.gov/sun/facts/>; PDG cosmology §22.2 (radioactive dating): chemical fractionation "helps pin down a precise age for the pre-Solar nebula of 4.6 Gyr" | High. Same page's intro says "4.5 billion-year-old"; use 4.6 |
| First stars | 3.16e15 (100 Myr) | "The first stars are thought to have formed as early as 100 million years after the big bang" | NASA Webb, *What Were the First Stars Like?*, <https://science.nasa.gov/mission/webb/science-overview/science-explainers/what-were-the-first-stars-like/> | Low-medium: a theoretical estimate, no Population III star has been observed. Say "~100–200 Myr". Planck's "Dark Ages" end at ~550 Myr is reionisation, not first stars |
| Recombination, CMB released | 1.20e13 (380 000 yr) | "decoupled 380,000 years after the Big Bang"; "cooled to around 3000 Kelvin" | ESA, *Planck and the cosmic microwave background*, <https://www.esa.int/Science_Exploration/Space_Science/Planck/Planck_and_the_cosmic_microwave_background>; CERN *The early universe*, <https://home.cern/science/physics/early-universe> ("It took 380,000 years …") | High. PDG cosmology §22.4: last scattering at z ≃ 1100, "when the age of the Universe was about 370,000 years" (1.17e13 s) |
| Big Bang nucleosynthesis | 180 (abundances fixed) | "synthesized at the end of the first three minutes"; abundances "essentially fixed by t ∼ 180 s"; n/p freeze-out at "T ∼ 1 MeV, corresponding to an age t ∼ 1 s" | PDG BBN review §24.1–24.3 (URL above); CERN: "Within minutes, these protons and neutrons combined into nuclei." | High. The span "1 s to ~20 min" is common in textbooks; the 20 min end is (unverified) here |
| Neutrino decoupling | 1 | PDG cosmology eq. 22.48: neutrinos in equilibrium for `T > (500 m_W^4/M_P)^(1/3) ∼ 1 MeV`; PDG BBN: decoupling temperature of left-handed neutrinos "3 MeV" | PDG cosmology §22.3.3; derived: T = 1 MeV, N = 10.75 gives 0.73 s; T = 3 MeV gives 0.08 s | Medium: order of magnitude. Say "~1 s" (0.1–1 s) |
| Quark-hadron transition (protons and neutrons form) | 2e-5 | Lattice QCD: "T_c(0) = (156.5 ± 1.5) MeV"; CERN: "A few millionths of a second later, quarks aggregated to produce protons and neutrons." | Bazavov et al. (HotQCD), *Chiral crossover in QCD at zero and non-zero chemical potentials*, Phys. Lett. B 795 (2019) 15, <https://arxiv.org/abs/1812.08235>; CERN early-universe page (above). Derived: 2.4e-5 s (N = 17.25) to 1.4e-5 s (N = 51.25) | Medium-high. It is a smooth crossover, not a sharp moment. "~1e-5 s" is fine |
| Electroweak transition (Higgs field switches on) | 1e-11 | "the cross-over is sharply defined at T_c = (159 ± 1) GeV" | D'Onofrio, Rummukainen, Tranberg, *The Sphaleron Rate in the Minimal Standard Model*, Phys. Rev. Lett. 113, 141602 (2014), <https://arxiv.org/abs/1404.3565>. Derived: 9.2e-12 s (N = 106.75) | Medium-high for the Standard Model; a crossover, not a phase transition. No agency page found stating the time; secondary reviews quote 1e-12 to 1e-11 s |
| Inflation | 1e-32 (end; start ~1e-36) | "from 10^-36 seconds after the Big Bang to a point between 10^-33 and 10^-32 seconds" | Secondary only (Wikipedia mirrors, Astronomy magazine Q&A, <https://www.astronomy.com/science/how-can-we-determine-that-inflation-occurred-from-10-35-to-10-32-second-after-the-big-bang-and-that-the-universe-grew-1026-in-that-time/>). ESA's Planck page says only "immediately after the hot Big Bang"; the PDG cosmology review gives no time | (unverified). Model-dependent: the energy scale of inflation is unknown (only an upper bound from the tensor-to-scalar ratio). Label it "hypothetical, ~1e-36 to 1e-32 s" |
| GUT scale (as a length) | 1.97e-32 m | `ħc = 197.3269804 MeV fm` (CODATA 2022, <https://physics.nist.gov/cgi-bin/cuu/Value?hbcmevf>); `ħc / 1e16 GeV` | Derived; 1e16 GeV as the GUT scale is the conventional figure (unverified, no source fetched) | Hypothetical: grand unification is not established. Show "~2e-32 m, hypothetical" |
| Planck epoch | < 5.39e-44 | `5.391 247(60) x 10^-44 s` | CODATA 2022 Planck time, <https://physics.nist.gov/cgi-bin/cuu/Value?plkt>; see [[sources/codata-2022-constants]] | Constant exact to 1e-5. That it marks the limit of known physics is a convention, not a measurement |

## Notes for the overlay

- Order of the markers, earliest first: Planck epoch, inflation (hypothetical), electroweak,
  quark-hadron, neutrino decoupling, nucleosynthesis, CMB, first stars, Sun and Earth, today.
- Before about 1e-11 s nothing is tested by experiment; the LHC reaches roughly that energy
  (13.6 TeV collisions, [[sources/cosmic-scale-references]]). Inflation and GUT markers should
  read as hypothetical.

## Contradictions

- Recombination: 380 000 yr (ESA, CERN, NASA) against "about 370,000 years" (PDG cosmology
  review). Both round the same last-scattering epoch; not worth a question page.
- Sun age: 4.6 Gyr and 4.5 Gyr on the same NASA page; 4.6 matches the PDG (radioactive dating) value.
