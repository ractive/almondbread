---
paths:
  - "kb/**"
---
# almondbread knowledge base (kb/wiki): link form and OKF rules

The full rules are in `kb/CLAUDE.md`; read them before touching `kb/`. Use the `hyalo` CLI
from the repository root (`.hyalo.toml` sets `dir = "kb/wiki"`; `hyalo -h`, `hyalo <cmd> -h`).

kb/wiki is an Open Knowledge Format (OKF) bundle. `.hyalo.toml` sets
`[lint] profiles = ["okf"]`; `hyalo lint --limit 0` must report no errors and no warnings on
every page before a commit. Never add `[lint] ignore` entries or switch rules off to get there.

- **Links in page bodies** are plain relative Markdown links that GitHub renders:
  `[Title](../techniques/bla.md)`. The path is relative to the linking page, `.md` included;
  a heading is its GitHub anchor (`../synthesis/benchmarks.md#as-built`). The link text is the
  target page's title or a short phrase that fits the sentence. No `[[wikilinks]]` or
  `![[embeds]]` (except inside code, where they are text). Do not run `hyalo links auto`: it
  writes `[[wikilinks]]`.
- **Inline citations**: `(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §BLA)`.
  Unsourced claims are marked `(unverified)`; own measurements give date, machine and build.
- **`# Citations`**: every page ends with a level-1 `# Citations` section, a list of links
  built only from what the page cites (its frontmatter `sources`, its `(src: ...)` pages,
  the external URLs it cites): `- [Title](../sources/x.md)` or `- <https://...>`. A `source`
  page lists the URL of the document it describes. Never invent a citation; a page that cites
  nothing says so in one entry linking to its own section where its basis is stated:
  `- [No cited source: our own design decision; its basis is the Context and Reasons above](#context)`.
- **Frontmatter lists** (`sources`, `depends-on`) hold OKF concept IDs, the page path
  without `.md`: `sources: ["sources/kf-manual"]`. hyalo does not track these as links: list
  each one in `# Citations` too, and after `hyalo mv` fix them by hand
  (`hyalo find --property 'sources~=old-name'`).
- **Reserved files**: `index.md` and `log.md` have no frontmatter (the root `index.md` only
  `okf_version`). Add a new page to the catalogue in `index.md`, then run
  `hyalo okf index --apply` (it maintains the `okf:index` region and each directory's
  `index.md`). Log with `hyalo okf log --action Ingest|Query|Research|Lint|Decision
  --message "Title: text" --apply` (newest first under `## YYYY-MM-DD`).
- Code comments cite pages as `kb: techniques/bla`; that is not a link, keep it.
- Checks: `hyalo lint --limit 0`, `hyalo find --broken-links`, `hyalo okf index` (dry run,
  reports stale index files).
