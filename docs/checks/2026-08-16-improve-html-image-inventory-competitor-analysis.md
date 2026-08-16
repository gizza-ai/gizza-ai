# html-image-inventory — competitor / reference scan (2026-08-16)

Scan run **before** implementation, to shape the parameter set, the column set, and the page copy.
Everything below is a paraphrase of publicly documented behaviour observed on the vendors' own
pages. **No competitor wording, copy, branding, or trademarked phrasing has been copied into this
repo**; product names appear only to attribute a capability claim.

## What was surveyed

| # | Reference | Input model | What it reports per image | Export | Notable limits |
|---|-----------|-------------|---------------------------|--------|----------------|
| 1 | Web Aloha "Image SEO Checker" (webaloha.co) | Public URL only | Alt presence/length, width+height, lazy-loading attribute, `srcset`, `<picture>`/`<source>` incl. WebP/AVIF sources, filename quality, format guessed from the URL | Per-image cards with severity colours + a heuristic 0–100 score | Server-side fetch; no JS execution (client-rendered images and CSS backgrounds missed); detailed list truncated at 100 items; does not download the bytes, so real dimensions/format/size are never verified |
| 2 | Sitechecker "Image Alt Tag Checker" (sitechecker.pro) | URL / whole-domain crawl, plus a browser extension | Alt present / empty / missing, `title` attribute, image size | Site-audit report (format unspecified) | Account + 14-day trial gate; no documented `srcset`, `<picture>`, `loading` or `decoding` reporting |
| 3 | Aspose "Image Alt Tag Checker" (products.aspose.app) | URL, file upload, **or pasted HTML** | WCAG 2.1 findings (H37/H2/H67/H86/G197), error/warning counts, offending code fragments | JSON / XML / TXT | Processes server-side (markup is uploaded); output is a conformance finding list, not an attribute table |
| 4 | PikaSEO "Image Alt Text Checker" (pikaseo.com) | Public URL only | Totals for images / with alt / missing alt / empty alt, per-image alt status in filterable tabs | CSV | URL-only, so it cannot inspect a staging page, an email template, or a snippet; alt only — no dimensions or loading hints |
| 5 | Scribely "Alt Text Checker" (scribely.com) | Public URL | Per-image list bucketed into "described" vs "needs attention" | Not documented | Accessibility framing only; no layout-shift / performance attributes |

Supporting references for *which* flags are worth raising (not competitors, but the standards the
flags are judged against):

- Lighthouse / PageSpeed raise **"Image elements do not have explicit `width` and `height`"** because
  an unsized image reserves no space and shifts the layout as it loads (a Cumulative Layout Shift
  contributor, and CLS is a Core Web Vital). Lighthouse issue #14449 records that the audit also
  accepts percentage values, which the HTML spec does not — the `width`/`height` **content
  attributes** must be valid non-negative integers.
- Lighthouse `image-alt` covers the missing-`alt` accessibility failure; an explicit `alt=""` is the
  spec-sanctioned marker for a decorative image and is *not* a defect.
- Documented attribute guidance: below-the-fold images want `loading="lazy"` + `decoding="async"`;
  the above-the-fold LCP image must **not** be lazy-loaded, and often wants `fetchpriority="high"`.

## Gaps in the field, and what this tool does about them

| Gap observed | Decision here |
|---|---|
| 4 of the 5 accept **only a public URL**. None of those can audit a snippet, a staging/authenticated page, a CMS template fragment, or an HTML email. | Input is **pasted HTML**. That is also the only honest option: this block is offline and has no network. |
| The one that accepts pasted HTML (#3) **uploads it to a server** for processing. | Runs entirely in the browser via WebAssembly (and locally via the CLI). The markup never leaves the machine. |
| Most report **alt only**. Only #1 reports dimensions, `loading`, and `srcset` — and it is URL-only. | One row per image carrying `src`, `srcset`, `sizes`, `alt`, `width`, `height`, `loading`, `decoding`, `fetchpriority`, `class`, `id`, `title` — the full attribute surface the backlog row asks for. |
| `<picture>` / `<source>` is handled by **only one** of the five. | `<picture><source>` candidates are inventoried as their own rows, tied to the `<img>` they belong to, with `media` and `type` so art-direction and WebP/AVIF fallbacks are visible. |
| Alt state is usually a **boolean**. Empty `alt=""` (decorative, correct) and a missing `alt` (a real failure) are frequently conflated, and #4 has to show them as separate buckets to compensate. | Three-state `alt_state`: `present` / `empty` / `missing`. Only `missing` is flagged by default; `flag_empty_alt` opts into auditing decorative images too. |
| Dimension checks (where they exist) treat any `width` attribute as a pass. | `width="50%"` / `width="auto"` are reported verbatim **and** flagged, because the HTML content attribute must be a non-negative integer — a percentage does not reserve layout space. |
| Outputs are HTML report cards or vendor-specific score JSON; only #4 exports CSV. | Three first-class outputs: `markdown` (a table to paste into a PR or ticket), `csv` (spreadsheet), `json` (scripting) — all from the same parse. |
| Scores are **heuristic and unexplained** (#1 starts at 100 and deducts). | No invented score. A factual summary line (totals, flagged counts) plus per-row issue tokens, each of which maps to a named spec/audit rule. |
| #1 truncates its detail list at 100 rows without a visible marker. | Hard cap of 2000 rows, and exceeding it is a loud error naming the cap, never a silent truncation. |

## Deliberately out of model (documented, not built)

These are real competitor features that cannot be delivered truthfully by an offline markup parser.
They are listed so nobody re-litigates them, and the page copy states the limitation plainly:

- **Fetching a URL / crawling a site** (#1, #2, #4, #5) — the block has no network access. Users
  paste markup (View Source, `curl`, or DevTools → Copy outerHTML).
- **Real file weight, real pixel dimensions, real format** — needs the image bytes. #1 concedes the
  same limitation even though it fetches the page. Attribute values are reported as written; they
  are never verified against the file.
- **Broken-image / 404 source detection** — requires a request per source.
- **JavaScript-rendered images and CSS `background-image`** — not present in static markup. Copying
  from DevTools (post-render DOM) instead of View Source is the documented workaround.
- **Alt-text quality judgement** ("low-quality description", keyword-stuffing) — a subjective model
  call, not a parse. Alt text is reported verbatim so a human can judge it; the tool does report the
  purely factual `alt` length and duplicate-alt grouping is left out rather than guessed at.
- **WCAG conformance verdicts** (#3) — a page-level conformance claim needs far more than the image
  subtree; the tool reports the facts the relevant techniques rest on instead of asserting a verdict.
