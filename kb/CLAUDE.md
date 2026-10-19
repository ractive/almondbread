# almondbread knowledge base

An LLM-maintained wiki (Karpathy "LLM wiki" pattern) about the maths and
engineering of fast deep-zoom fractal rendering: perturbation, iteration
skipping, numerics, colouring, GPU (Metal, Apple Silicon) and existing
renderers. It feeds almondbread, the macOS/Metal fractal explorer in the
parent directory. Code there cites pages as `kb: techniques/bla`; keep page
paths stable.

## Layers

- `raw/`: the source material, unchanged: PDFs, saved web pages and their
  `.txt` conversions, and raw research notes (`raw/research/<title>/`).
  Never edit a raw file. Large or copyrighted files are not committed
  (see `.gitignore`); `sources/manifest.json` says how to get them back.
- `sources/manifest.json`: one entry per raw file: path, title, authors,
  year, URL, size, SHA-256, terms, method (`fetch`, `zip-member`, `manual`,
  `derived`), and the `wiki` pages that describe it.
- `scripts/fetch-sources`: downloads the `fetch` entries into `raw/` and
  checks hashes (`--dry-run`, `--check`, `--derive`). Run it from anywhere.
- `wiki/`: the pages. The only layer you write prose in.
- `CLAUDE.md`: this file. It evolves with the wiki.

## Tooling

Use the `hyalo` CLI (not Read/Grep/Glob) for wiki operations: search,
frontmatter, tags, links, moves, lint. Run it from the repository root;
`../.hyalo.toml` sets `dir = "kb/wiki"`, so pass paths relative to
`kb/wiki/` (`hyalo read techniques/bla.md`). Orient with `hyalo summary`,
then `hyalo read index.md`. Rename or move pages only with `hyalo mv` (it
rewrites links). Create pages with Write, edit prose with Edit, everything
else through hyalo. `hyalo lint` must be clean before a commit.

## Code licences

Several renderers are GPL/AGPL (Kalles Fraktaler 2+, Fraktaler 3, and
others; the `implementations/` page records each licence). Algorithms and
facts are free to use; code is not. Wiki pages describe algorithms in our
own words, with formulas and our own pseudocode, never pasted code from a
copyleft project. Cite which file or post a fact came from.

## Wiki layout

| Directory | Page type | Purpose |
| --- | --- | --- |
| `sources/` | `source` | One page per document or web source: what it is, what it covers, how reliable, where the useful parts are. |
| `techniques/` | `technique` | One page per algorithm or trick: perturbation, rebasing, BLA, series approximation, floatexp, interior detection, distance estimation, colouring, frame reuse, ... Formulas, pseudocode, costs, pitfalls. |
| `formulas/` | `formula` | One page per fractal formula (mandelbrot, burning-ship, multibrot, ...): iteration, perturbed form, BLA form, what works. |
| `implementations/` | `implementation` | One page per existing renderer: algorithms, precision types, GPU use, licence, status. |
| `platform/` | `platform` | Apple GPU and Metal facts, CPU arbitrary precision (MPFR/GMP), float-float emulation. |
| `decisions/` | `decision` | Design decisions for almondbread, ADR style. |
| `questions/` | `question` | Open questions, contradictions between sources, things to benchmark. |
| `synthesis/` | `synthesis` | Answers worth keeping: research reports, comparisons, roadmaps. |
| `plans/` | `plan` | One page per milestone plan (fractal types F1–F8, optional looks, parked ideas): goal, scope, non-goals, design sketch, affected modules, verification, review gate, risks, open questions, effort breakdown. Listed by `hyalo find --view plans`. |
| `index.md` | `meta` | Catalogue of every page with a one-line summary, by directory. Updated on every ingest. |
| `log.md` | `meta` | Append-only. Entry prefix `## [YYYY-MM-DD] ingest\|query\|research\|lint\|decision \| title`. |
| `overview.md` | `meta` | The current big picture in one page. |

## Page conventions

- Filenames: kebab-case, no dates in names.
- Frontmatter per type is enforced by `../.hyalo.toml` (`hyalo types list`,
  `hyalo types show <type>`). Common keys: `title`, `type`, `tags`,
  `status` (`stub|draft|reviewed`; questions `open|answered`; sources
  `unread|skimmed|digested`), `sources` (list of `[[sources/...]]` links).
- `plan` pages (`plans/<milestone-slug>.md`, one per milestone in `roadmap.md`):
  frontmatter `milestone` (e.g. `F3`), `status` (`proposed|planned|in-progress|done|dropped`),
  `priority` (number, the order), `effort` (`S` days, `M` 1–2 weeks, `L` 3–6 weeks,
  `XL` months), `appeal` (`low|medium|high`), `audience` (list: `casual`, `education`,
  `artists`, `deep-zoom-creators`, `enthusiasts`, `developers`), `depth` and `reuse` (text),
  `realtime-ipad` (`yes|no|likely|unknown`), `depends-on` (list of `[[plans/...]]`), `risks`
  (list), `sources` (any wiki links), `tags`, `created`, `updated`. Required body sections
  (`## ` headings, enforced): Goal, Scope, Non-goals, Design sketch, Affected modules (our
  files), Verification (acceptance criteria, including how to prove the Mandelbrot fast path is
  unaffected: bit-identical fields and a `bench` A/B ≈ 1.0), Review gate (what the user tries
  before the next step), Risks, Open questions, Effort breakdown. Keep `status` in sync with
  the roadmap row; weak evidence is marked `(unverified)` as everywhere.
- Every technical claim carries a citation inline:
  `(src: [[sources/mathr-deep-zoom-theory]] §BLA)`. Unsourced claims are
  marked `(unverified)`. Our own measurements say so, with date, machine
  and build (`measured 2026-10-14, M1 Pro, release`).
- A `source` page's `raw` field is its path under `kb/raw/` as in the
  manifest; for material not saved, give the URL and write `raw: "web"`.
- Formulas in LaTeX-ish inline code or `$...$`; pseudocode in fenced
  blocks, our own.
- Link liberally with `[[dir/page]]`. A link to a missing page is a todo;
  `hyalo find --broken-links` lists them.
- Contradictions between sources go on the page under `## Contradictions`
  and get a `questions/` page.
- Plain, short English.

## Operations

**Ingest** a source: save it to `raw/` (and a `.txt` conversion for HTML
or PDF where possible), add a manifest entry, write its `sources/` page
(status `digested`), extract facts into `techniques/`, `formulas/`,
`implementations/` or `platform/` pages, raise `questions/` for anything
unclear, update `index.md`, append to `log.md`.

**Research**: deep-research runs save their notes under
`raw/research/<title>/` (unchanged afterwards) and the final report as a
`synthesis/` page. Then ingest: one `sources/` page per important cited
document, facts into topic pages, index and log.

**Query**: read `index.md`, then the pages; answer with citations. If the
answer is worth keeping, file it under `synthesis/` or `questions/` and
log it.

**Lint**: `hyalo lint`, `hyalo find --orphan`, `hyalo find --broken-links`,
`hyalo find --view stubs`, `hyalo find --view open-questions`. Look for
stale or contradicting claims and sources still `unread`. Log the pass.
