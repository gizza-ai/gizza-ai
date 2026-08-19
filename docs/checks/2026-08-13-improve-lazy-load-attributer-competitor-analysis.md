# lazy-load-attributer — competitor analysis (2026-08-13)

Scan run **before** implementing, per `create-next-tool` step 4. Findings are paraphrased
observations of publicly documented behaviour; no competitor copy, branding, or trademarks are
reproduced, and out-of-model items are listed, not built.

## Scope of the tool

Paste HTML → get the same HTML back with `loading="lazy"` and `decoding="async"` added to `<img>`
and `<iframe>` tags that lack them. Pure, deterministic, in-browser; no network, no image files
read.

## Competitors reviewed

### 1. WordPress core — `wp_img_tag_add_loading_optimization_attrs()`
The reference server-side implementation: the canonical "rewrite HTML, add the attributes" pass
that runs on every WordPress page.

- Adds three attributes: `loading` (default `lazy`), `decoding` (default `async`), and
  `fetchpriority="high"` for the image it judges to be the LCP candidate.
- **Never overrides an attribute that is already present** — an author-set `loading="eager"` wins.
- **Skips images with no `src`** — there is nothing to defer.
- Skips `loading` / `fetchpriority` on images missing both `width` and `height` (layout-shift
  guard); `decoding` is still applied.
- The first in-content image is treated as above-the-fold: it is *not* lazy-loaded, and is the
  `fetchpriority="high"` candidate.
- Filters let integrators override the `loading` value (`lazy`/`eager`) and the `decoding` value
  (`async`/`sync`/`auto`) per image.

### 2. WP Rocket LazyLoad (`rocket-lazy-load` plugin)
The mainstream configurable lazy-load product; the source of the de-facto exclusion vocabulary.

- Covers **images and iframes** as separately togglable classes of element (iframe lazy-loading can
  be disabled on its own).
- Exclusion is the headline feature: `data-no-lazy="1"` on a tag, plus the cross-plugin
  interoperability markers `skip-lazy` (class) and `data-skip-lazy` (attribute), plus
  filter-driven attribute/src pattern exclusion lists.
- Documents an "exclude the first N images" workflow (helper plugin + attribute pattern) so the
  LCP image is never deferred.
- Also offers a JS runtime (`data-lazy-src` swap + `rocket-lazyload` class), a viewport
  `threshold`, background-image lazy-loading on container elements, and YouTube-embed thumbnail
  replacement.

### 3. Lazy-loading image snippet generator (en.ud5.com)
The browser-tool class this page competes with directly: paste/enter details, get markup back.

- Emits a single `<img>` tag carrying `loading="lazy"`, plus an optional low-quality placeholder.
- Single-tag generator only — it builds one snippet rather than rewriting a document, and offers
  no exclusions, no iframe handling, and no idempotency guarantee.

## Table stakes → decision

Every item below lands in the descriptor or in the explicit out-of-model list. Nothing dropped
silently.

| Table stake | Seen in | Decision |
| --- | --- | --- |
| Add `loading="lazy"` to `<img>` lacking it | all three | **In model** — core behaviour |
| Add `decoding="async"` to `<img>` lacking it | WP core | **In model** — `decoding` param |
| Handle `<iframe>` too, togglable separately | WP Rocket | **In model** — `targets` enum |
| Never override an attribute already present (idempotent re-runs) | WP core, WP Rocket | **In model** — always on, documented |
| Skip images with no `src` | WP core | **In model** — always on |
| Leave the first N images alone (LCP / above the fold) | WP core, WP Rocket | **In model** — `skip_first` integer |
| Mark those first images `loading="eager"` explicitly | WP core | **In model** — `eager_first` boolean |
| `fetchpriority="high"` on the LCP image | WP core | **In model** — `fetchpriority_first` boolean |
| Honour `skip-lazy` / `no-lazy` classes and `data-skip-lazy` / `data-no-lazy` attributes | WP Rocket | **In model** — `respect_skip_markers` boolean, default on |
| Choose the `decoding` value (`async`/`sync`/`auto`) or skip it | WP core filters | **In model** — `decoding` enum incl. `none` |
| Report how many tags were changed / skipped | WP Rocket UI counters | **In model** — `output = report` |
| Preserve the rest of the document byte-for-byte | all | **In model** — only the target tags are rewritten |
| One-click presets for common configurations | WP Rocket settings screen | **In model** — `[[example]]` preset chips on the page |

## Out of model (listed, not built)

- **JS-runtime lazy loading** (`data-lazy-src`/`data-src` swap + a companion loader script,
  `noscript` fallbacks, IntersectionObserver `threshold`/`rootMargin`). Requires shipping and
  wiring a runtime script alongside the markup; this tool emits self-contained standard HTML that
  needs no JavaScript.
- **Background-image lazy loading** on `div`/`section`/`figure` containers. Needs CSS parsing and
  a JS runtime to swap `background-image`; there is no native HTML attribute for it.
- **YouTube / video-embed thumbnail placeholders.** Needs network fetches of poster images and
  provider-specific URL knowledge.
- **`width`/`height` inference and `srcset`/`sizes` generation.** Requires reading the actual image
  files to learn their intrinsic dimensions; this tool never touches the network or the filesystem.
  (Note: WP core's "skip images missing width/height" rule is therefore also not replicated as a
  default — the caller's markup is respected as-is.)
- **True above-the-fold detection.** Requires rendering the page with a real layout engine.
  `skip_first` is the deterministic, markup-only approximation that both WP core and WP Rocket
  ship as the practical answer.
- **Whole-site / crawl-based application.** This is a single-document transform; batch application
  is an invocation pattern (CLI loop), not a distinct capability.

## UX controls adopted

- `targets`, `decoding`, and `output` render as `<select>` menus with friendly `[input.labels]`.
- `respect_skip_markers`, `eager_first`, and `fetchpriority_first` render as checkboxes
  (`respect_skip_markers` defaults on).
- `skip_first` uses `kind = "slider"` (0–10, step 1) — a small bounded range where dragging beats
  typing, matching the "exclude the first N images" control competitors expose.
- Four `[[example]]` preset chips mirror the configurations competitors ship as presets:
  everything lazy, LCP-safe (skip the first image + `fetchpriority`), images only, and the
  change report.
