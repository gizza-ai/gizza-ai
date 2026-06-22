# citation-generator — competitor analysis (2026-06-22)

## What the tool does

Formats a single bibliographic reference into **APA 7th**, **MLA 9th**,
**Chicago 17th** (notes-bibliography entry), or **Harvard** (author-date) style
from structured fields: `style`, `title`, `authors`, `year`, `container`
(journal/website/book), `publisher`, `volume`, `issue`, `pages`, `url`, `doi`,
`accessed`. Pure-Rust, deterministic, runs in chat / CLI / page. No lookup, no
network — a formatter, not a metadata fetcher.

## Top competitors surveyed

1. **Scribbr** — APA, MLA, Chicago, Harvard; strong accuracy; export to Word/LaTeX.
2. **ZoteroBib (zbib.org)** — 10,000+ CSL styles; free, no account; lookup by ID/URL.
3. **Cite This For Me** — APA, MLA, Harvard, Chicago, ASA, IEEE, AMA.
4. **QuillBot Citation Generator** — APA, MLA, Chicago; web-based.
5. **Zotero (desktop)** — full reference manager, Word/LibreOffice/Docs integration.

Sources:
- https://www.scribbr.com/citation/generator/
- https://zbib.org/
- https://citeme.app/learn/best-free-citation-generators
- https://quillbot.com/citation-generator
- https://researcher.life/blog/article/top-6-free-citation-generator-tools/

## Gap diff and ranking (fit-to-model)

| Competitor capability | In gizza model? | Action |
|---|---|---|
| APA / MLA / Chicago styles | Yes (built) | Shipped |
| **Harvard (author-date) style** | Yes — pure string formatting | **Added this pass** (4th style; high value, common with UK/AU students) |
| Multiple-author handling, organization authors, n.d./no-date | Yes | Shipped (`;`/`and` split, et al. for MLA 3+, `(n.d.)`/`(no date)`) |
| DOI preferred over URL, auto `https://doi.org/` prefix | Yes | Shipped |
| Sentence-case (APA) vs title-case (MLA/Chicago/Harvard) headings | Yes | Shipped (acronyms/proper nouns preserved) |
| Auto-lookup metadata from a DOI / URL / ISBN | **No** — needs a network fetch + external DB (CrossRef/OpenAlex); chat block can't fetch, and it would no longer be deterministic | Out of model — not built |
| Italic journal/book titles | **No** — output is plain text; no rich-text surface on the page/chat/CLI | Out of model — documented in content.md/skill ("apply italics after pasting") |
| Export to Word / Google Docs / .bib | **No** — UI/integration feature, no surface | Out of model — not built |
| IEEE / AMA / ASA / 10,000 CSL styles | Partial fit — each is pure formatting but a long tail; the 4 added cover the dominant student/academic demand | Deferred — could add IEEE later as a follow-up; not a blocking gap |
| Multi-source bibliography list (cite many at once) | Single-source by design (one descriptor input shape) | Out of scope for this tool |

## Gaps closed this pass

- **Added Harvard (author-date) style** — `style='harvard'`, with APA-style
  initials joined by "and", single-quoted article titles, `vol(issue)`,
  `pp. pages`, and `Available at: <doi/url> (Accessed: <date>)`. Reaches parity
  with Scribbr / Cite This For Me / ZoteroBib on the four most-requested styles.
- Updated descriptor enum + drift-guard schema, manifest, page meta tags/title,
  and content.md; added core unit tests (`harvard_article`, `harvard_book`) and
  a Playwright Harvard-book page assertion.

## Verification

- `cargo test --workspace`: 16 core tests + 1 chat drift-guard schema test pass.
- `wafer build`: chat block validates and instantiates (334 KiB).
- CLI (`gizza tool citation-generator …`): APA / MLA / Chicago / Harvard all
  produce correct entries; unknown style → error + exit 1.
- Playwright (`tool-page-citation-generator.spec.ts`): 3/3 pass (MLA article,
  APA book, Harvard book).

## Out-of-model items (explicitly NOT built)

Metadata auto-lookup (needs network + external DB, breaks determinism), rich-text
italics (no rich-text output surface), document/.bib export (UI integration), and
the long tail of CSL styles beyond the four shipped. No competitor copy, branding,
or trademarks were used.
