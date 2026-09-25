# html-accessibility-checker — competitor analysis (2026-08-29)

Scan run **before** implementation, per `/improve-tool` Phase 2–3. One web search
(`online HTML accessibility checker paste HTML code a11y validator tool`), then the top three
reachable paste-HTML competitors were read. Everything below is **paraphrased**; no competitor
copy, wording, branding, or assets were reused.

## Duplicate check (Phase 0)

`ls blocks | grep -Ei 'html|access|a11y|lint|validate'` plus a read of the closest neighbours:

| Existing block | What it actually does | Overlap |
| --- | --- | --- |
| `html-validate` | Syntax/nesting scanner — unterminated tags, unclosed elements, crossed nesting | none (no a11y rule) |
| `html-outline-analyzer` | Heading outline + literal tag counts | partial: heading order only |
| `html-image-inventory` | Table of every image source + attributes | partial: missing-`alt` only |
| `html-form-field-extractor` | Enumerates form controls + labels | partial: label data, no rule verdict |
| `seo-audit` | Scored **SEO** report: title/meta length, canonical, Open Graph, alt, lang | partial: alt + lang + heading order, framed as SEO |
| `color-contrast-checker` | Contrast ratio of two colour values | none (takes colours, not HTML) |

Verdict: **not a duplicate.** No block applies a WCAG rule catalogue to pasted markup. The
a11y-specific rules — unlabeled form controls, empty/generic link and button names, `iframe`
titles, duplicate `id`s, positive `tabindex`, `aria-hidden` wrapping focusable content, invalid
ARIA roles, tables without headers, zoom-blocking viewport, autoplay media, missing captions —
exist nowhere in the repo. The three-way overlap (alt / lang / heading order) is the intersection
every a11y checker shares with every SEO checker, not the product.

## Competitors read

### 1. ValidateHTML — accessibility checker (validatehtml.com/accessibility-checker)
Paste-HTML or URL input, single-pass. Six rules: missing `alt`, heading hierarchy, missing form
labels, missing `lang`, empty links/buttons, missing `<title>`. Results carry a two-tier severity
(error vs warning). No user-facing settings — no WCAG level picker, no severity filter, no output
format. Eight FAQ entries covering WCAG versions, ARIA, and how it differs from a validator. Paid
tier upsells recurring monitoring (server-side; out of model here).

### 2. FixTools — HTML accessibility checker (fixtools.io/html/html-accessibility-checker)
Explicitly targets WCAG 2.1 A/AA, in-browser only ("your HTML never leaves your device"). Rules
span alt text, `lang`, heading order, ARIA labels on interactive elements, form labels, semantic
structure, plus non-a11y extras (charset/viewport/description meta, `rel="noopener"` on
`target="_blank"`, unclosed tags). Results split three ways — **errors / warnings / suggestions** —
with a WCAG guideline reference and a fix hint per finding. UX: a "paste demo" button that
pre-fills a sample document, a clear button, one primary action button, 11 FAQ entries. WCAG level
is fixed at 2.1 AA (stated, not selectable).

### 3. AccessibilityChecking.com
Paste-into-editor input, no storage. Checks WCAG 2.1 + Section 508: non-descriptive link text,
missing/poor alt, semantic elements, form labelling, colour contrast, keyboard navigation.
Notable: it is the only one of the three with a **selectable conformance level — A, AA or AAA,
defaulting to AA** — plus an exclusion list for custom elements. Findings come with step-by-step
remediation text and a guided "repair" affordance. No documented export format.

## Table stakes → decision

| # | Table stake (from ≥1 competitor) | Fit | Shipped from day one |
| --- | --- | --- | --- |
| 1 | Missing `alt` on images | in-model | `img-missing-alt` (also `<input type=image>`, `<area>`) |
| 2 | Non-descriptive `alt` (filename/placeholder) | in-model | `img-alt-filename` |
| 3 | Missing `lang` on `<html>` | in-model | `missing-lang` (+ `invalid-lang` for values like `english`) |
| 4 | Missing `<title>` | in-model | `missing-title` |
| 5 | Heading hierarchy (no h1, several h1, skipped level, empty) | in-model | `heading-no-h1`, `heading-multiple-h1`, `heading-skipped-level`, `heading-empty` |
| 6 | Unlabeled form controls | in-model | `input-missing-label` (`for`/wrapping/`aria-label`/`aria-labelledby`/`title`), `label-orphan` |
| 7 | Empty links and buttons | in-model | `link-empty`, `button-empty` |
| 8 | Non-descriptive link text | in-model | `link-generic-text` (SC 2.4.9, so AAA-gated — see level param) |
| 9 | ARIA labels on interactive elements | in-model | covered by 6/7 + `iframe-missing-title`, `invalid-role`, `aria-hidden-focusable` |
| 10 | Three-tier severity: error / warning / suggestion | in-model | every rule carries one; `min_severity` filters |
| 11 | WCAG guideline reference per finding | in-model | each rule prints its success criterion + level |
| 12 | Selectable conformance level A / AA / AAA, default AA | in-model | `level` enum, default `aa` |
| 13 | Fix hint per finding | in-model | every message states what was expected and how to fix it |
| 14 | Sample/demo prefill button | in-model | three `[[example]]` chips (clean page, broken page, JSON) |
| 15 | Line numbers on findings | in-model | 1-based line + column on every issue |
| 16 | Score / summary counts | in-model | 0–100 weighted score + per-severity counts + checks-run/passed |
| 17 | `rel="noopener"` on `target="_blank"` | in-model | `blank-target-no-rel` (suggestion) |
| 18 | Table headers, tabindex, autoplay, captions, zoom-blocking viewport, duplicate ids | in-model (beyond table stakes) | `table-missing-header`, `positive-tabindex`, `autoplay-media`, `video-missing-captions`, `viewport-zoom-blocked`, `duplicate-id`, `focus-outline-removed`, `missing-main` |
| 19 | Machine-readable export (none ship it) | in-model | `format` = text / markdown / json / csv — our differentiator |
| 20 | Passing checks shown, not only failures | in-model | `show_passed` |
| 21 | Colour contrast | **out of model** | needs the CSS cascade + rendering; `color-contrast-checker` covers the colour-pair case |
| 22 | Keyboard/focus-order & focus-indicator testing | **out of model** | needs a live browser and a focus walk |
| 23 | Fetch-by-URL input | **out of model here** | this is a pure browser-local block; `web-fetch` is the URL surface |
| 24 | Auto-repair button | **considered, rejected** | rewriting someone's markup from a heuristic scan is not safe to do silently |
| 25 | Recurring monitoring / weekly re-scan | **out of model** | requires a server + account |
| 26 | Section 508 mapping | **considered, rejected** | duplicates the WCAG mapping already printed; adds schema noise |

## UX patterns adopted (idea-level only, original copy)

- Three-tier severity vocabulary (error / warning / suggestion) — matches how a11y reports are read.
- Demo prefill: implemented as declarative `[[example]]` chips rather than a bespoke button.
- Per-finding remediation sentence, not a bare rule name.
- An honest coverage caveat in the report footer and the page copy: automated rules catch only part
  of WCAG; contrast, focus order, and alt *wording* still need a human.

## Gaps we ship that no scanned competitor does

- Four output formats including JSON and CSV (all three competitors are HTML-only).
- A selectable `min_severity` filter (only severity *display* exists elsewhere).
- `show_passed` — the checks that passed, not just the failures.
- A `max_issues` cap with an explicit truncation notice instead of a silent cut-off.
- Everything runs locally as wasm with no upload, no account, and a CLI/JSON surface for CI use.
