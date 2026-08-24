# html-outline-analyzer — competitor analysis (2026-08-20)

Scan run before implementing the tool (build + improve passes are merged for a
backlog build). Everything below is **paraphrased** from public tool pages — no
competitor copy, naming, or branding is reproduced or reused.

## Field scanned

One web search for "HTML outline analyzer / heading structure H1–H6 / tag
counter", then the top real tools were skimmed. The category is crowded and
converged; the surveyed set:

| # | Competitor (category) | Input | Shape of the answer |
|---|---|---|---|
| 1 | SEO-suite heading structure analyzer | URL fetch | Summary cards (total headings, H1 count, errors, warnings) + heading list in document order |
| 2 | Heading structure checker (SEO) | URL fetch | Renders the H1–H6 tree as the markup defines it; flags skipped levels, multiple top-level headings, empty headings |
| 3 | H1 / heading checker (SEO) | **URL or pasted HTML** | Per-heading order, text, text length, id, visibility; flags missing/multiple H1, skipped levels, empty headings, duplicate heading text; also scores H1↔title overlap |
| 4 | Accessibility heading analyzer (WCAG) | URL (best-effort) or pasted HTML | Visual heading tree, error/warning flags, cites WCAG 1.3.1 / 2.4.6 / 2.4.10; client-side only, no upload |
| 5 | Heading tag analyzer (SEO) | URL fetch | Lists every heading tag in the order it appears |
| 6 | Heading tag checker (SEO) | URL fetch | H1–H6 with counts and order; **CSV / JSON export** |

Separately, the "HTML tag counter" niche (the second half of the backlog row)
is served by small utilities that report a per-tag frequency histogram and a
total element count for a document.

## Table stakes extracted (and what we did with each)

| Table stake | Decision |
|---|---|
| Full H1–H6 outline in document order, nested by level | **Built** — `format=tree` (indented), `markdown` (nested bullets), `json` (nested `children`), `csv` (flat rows) |
| Total heading count + per-level counts | **Built** — summary line, `h1:…  h2:…` breakdown, present in every format |
| Flag missing H1 | **Built** — `no-h1` (error) |
| Flag multiple H1 | **Built** — `multiple-h1` (warning) |
| Flag skipped levels (h2 → h4) | **Built** — `skipped-level` (error), names both the previous heading's line and the expected level |
| Flag empty headings | **Built** — `empty-heading` (error) |
| Flag duplicate heading text | **Built** — `duplicate-text` (warning), lists the lines that share the text |
| Report each heading's text, length, id | **Built** — all three, in every format |
| Report heading visibility | **Built, scoped** — `hidden-heading` (warning) from `hidden`, `aria-hidden="true"`, and inline `display:none` / `visibility:hidden`. Stylesheet-driven hiding is out of model (no CSS cascade) and is stated as a limit on the page |
| Accept pasted HTML (drafts, staging, page builders) | **Built** — the only input mode |
| CSV / JSON export | **Built** — `format=csv` and `format=json` |
| Client-side, nothing uploaded | **Built** — the whole family runs in the sandbox/browser |
| Element / tag frequency counts | **Built** — the tag histogram (`top_tags`), which none of the heading analyzers above ship and which the backlog row asks for |

## Gaps we go beyond on

- **1-based line numbers on every heading and every issue.** None of the
  surveyed heading tools locate a finding in the source; they show text only.
  Our scanner tracks lines, so an issue is directly navigable.
- **Element/tag histogram alongside the outline** — the surveyed heading tools
  and the tag-counter utilities are disjoint products; this row asks for both.
- **Source-fidelity counts.** Counting is done over the literal markup rather
  than a normalized DOM, so implied `<html>`/`<head>`/`<body>` elements a
  parser would invent are not counted as if the author wrote them.
- **Level window** (`min_level` / `max_level`) to focus on, e.g., just h2–h3.

## Considered, not built

- **URL fetching.** Out of model for a pure block: gizza pure tools take pasted
  input and run locally with no network. The pasted-HTML mode is what the
  URL-based competitors' own "paste" mode is, and it also covers drafts,
  staging, and page-builder markup that a fetcher cannot reach. (The repo's
  existing fetch-a-page tool, `css-select-extract`, covers the fetch shape.)
- **H1 ↔ `<title>` keyword/lexical-overlap scoring.** In model, but it belongs
  to the existing `seo-audit` block (which already checks title, meta
  description, H1 usage and skipped levels as part of a 0–100 score).
  Duplicating it here would blur two tools; rejected on scope.
- **Anchor/slug generation for each heading.** In model, but that is exactly
  what the existing `toc-generator` block does (GitHub-style anchors, nested
  link list). This tool reports the `id` that is actually in the markup instead.
- **JavaScript execution / post-render DOM.** Out of model everywhere — every
  competitor surveyed states the same limit. Stated on our page too.
- **Accessibility scoring / WCAG grade.** Rejected: a letter grade over four
  heading checks is not a defensible accessibility verdict; the issue list with
  explicit severities is the honest form.

## No-copy confirmation

No competitor copy, headline, FAQ text, class name, or branding was copied.
Feature names used here (`skipped-level`, `duplicate-text`, …) are our own
issue codes. Page copy is original and brand-free per the repo's hygiene gate.
