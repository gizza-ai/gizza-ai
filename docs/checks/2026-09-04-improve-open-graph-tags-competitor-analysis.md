# open-graph-tags — competitor analysis (2026-09-04)

Scan run **before** implementing, per `/create-next-tool` step 4. One WebSearch for
"Open Graph meta tag generator / Twitter Card generator", then the top 3 reachable real
tools were skimmed. All notes below are **paraphrased** — no competitor copy, branding,
or trademarks were reproduced, and no out-of-model feature was built.

## Competitors skimmed

| # | Tool | URL | Notes |
|---|------|-----|-------|
| 1 | Superframeworks Open Graph generator | superframeworks.com/tools/open-graph-generator | Reachable. Strongest on defaults + per-block copy. |
| 2 | Meta Tag OGP Generator (hidekazu-konishi.com) | hidekazu-konishi.com/tools/meta_tag_ogp_generator_tool.html | Reachable. Deepest schema: locale list, schema.org itemprop, quick-fill templates. |
| 3 | DeepakNess OG meta generator | tools.deepakness.com/og-meta-generator/ | Reachable. Minimal field set; character counters. |

(No competitor was unreachable, so no replacement was needed.)

## Table-stakes parameters observed

| Capability | Seen on | Our decision |
|---|---|---|
| Page title (required) | 1, 2, 3 | **In model** — `title`, required. |
| Description | 1, 2, 3 | **In model** — `description`. |
| Canonical page URL | 1, 2, 3 | **In model** — `url`. |
| Image URL | 1, 2, 3 | **In model** — `image`. |
| Site name | 1, 2, 3 | **In model** — `site_name`. |
| `og:type` dropdown | 1 (website/article/product/video/profile), 2 (+ `video.other`, `music.song`, `book`) | **In model** — `og_type` as `Param::enumv`, union of both lists. |
| `twitter:card` dropdown | 1 (summary/summary_large_image/player), 2 (+ `app`) | **In model** — `twitter_card` as `Param::enumv`, union. |
| Twitter handle | 1, 3 | **In model** — split into `twitter_site` + `twitter_creator` (the two distinct tags). |
| `og:locale` dropdown | 2 (16 locales) | **In model** — `locale`, default `en_US`. Free string, not an enum: the locale space is open-ended (any `xx_YY`), so a 16-item enum would wrongly reject valid values. |
| Author name | 2 | **In model** — `author` (emits `meta name=author`, plus `article:author` for article types). |
| Image alt text | none of the three | **In model anyway** — `og:image:alt` / `twitter:image:alt` is in the OG spec and is an accessibility win. |
| Image width/height tags | none of the three (all only *advise* 1200×630) | **In model anyway** — `image_width` / `image_height` emit `og:image:width|height`, which lets crawlers lay out the card before fetching the image. |
| Section comment blocks in output | 2 (checkbox) | **In model** — `group_comments`, default on. |
| schema.org `itemprop` tags | 2 | **In model** — `include_schema`, default off. |
| Emit only some blocks | 1 (copy OG / Twitter / both separately) | **In model, re-expressed** — copy-splitting is a UI affordance; we expose it as generation flags `include_basic` / `include_twitter` so the CLI and chat get it too. |
| Validation warnings for missing/oversized fields | 2 | **In model** — `warnings`, default on: appends an HTML-comment check block (title/description length, relative image URL, large-image card with no image, missing url/site_name). |
| Character-limit guidance (60 title, 100–200 desc) | 1, 2, 3 | **In model as copy + checks** — stated on the page, enforced as warnings, never as a hard cap. |
| Absolute-URL requirement for images | 1 | **In model** — a relative `image` raises a warning naming the expected form. |

## UX control patterns observed → our equivalents

| Pattern | Seen on | Our decision |
|---|---|---|
| Live social-card preview (Facebook / X / Google SERP renderings) | 1, 2 | **Out of model** — rendering a faithful third-party card chrome means shipping imitations of other companies' UI. Listed, not built. |
| Live character counters | 1, 2, 3 | **Out of model for the generic form** — the shared generator has no counter control kind, and adding one for a single tool would be a per-tool hack. Covered instead by the `warnings` check block, which reports the actual counts. |
| Quick-fill templates / presets (website, article, product, video, profile) | 2 | **In model** — shipped as `[[example]]` preset chips on the page (article, product, video, profile-shaped presets). |
| Copy button | 1, 2, 3 | **Already platform** — every text tool gets Copy + Reset + Download from the shared runtime. |
| Export as `.html` snippet | 2 | **Already platform** — `format = "text"` pages get a Download link. |
| Dropdowns for fixed choices | 1, 2 | **In model** — `og_type`, `twitter_card` are `Param::enumv` → real `<select>`s. |

## Out-of-model (considered, not built)

- **Live Facebook / X / Google preview cards** — would require reproducing competitors' and
  platforms' card chrome; rejected on the no-copy rule, not on feasibility.
- **Live character counters in the form** — needs a new shared control kind; the check block
  reports the same counts without a per-tool hack.
- **Fetching an existing page's tags to pre-fill the form** — needs a network fetch from the
  browser; cross-origin and out of the pure-compute page model. `seo-audit` already covers
  auditing HTML you paste in.
- **Image upload / hosting for `og:image`** — needs storage; gizza is browser-local, no backend.
- **Facebook Sharing Debugger / Card Validator round-trips** — third-party APIs, out of model.

## Relationship to existing blocks (not a duplicate)

`blocks/seo-audit` *consumes* finished HTML and grades it (its `check_open_graph` looks for
`og:title`/`og:description`/`og:image` and `twitter:card` in pasted markup). This tool is the
inverse: it *produces* the markup from typed fields. Different input, different output,
complementary pair — not a semantic near-dup.
