# json-ld-generator — competitor analysis (2026-06-23)

Schema.org **JSON-LD structured-data generator**: pick a `@type`, enter `name = value`
field pairs, get valid JSON-LD (optionally wrapped in a `<script type="application/ld+json">`
tag). Three surfaces: chat skill, `gizza tool json-ld-generator`, and the `/tools/json-ld-generator/` page.

## Surfaces verified

- **Chat block** — `wafer build` validates `target/block.wasm` (OK, 351 KiB).
- **CLI** — Article (nested `author`→Person + `keywords` array), Product (`offers.price`→Offer,
  numeric), FAQPage (Q&A → `mainEntity`, `wrap_script`), Review (`reviewRating`→Rating), and the
  unknown-type error path all produce correct output / exit non-zero.
- **Page** — Playwright (4 specs): Article, Product numeric typing, FAQPage + wrap-script
  checkbox, and a `?schema_type=&fields=` deep-link prefill. All pass.
- Unit tests: 15 core + 1 descriptor drift-guard.

## Top competitors

| Tool | Notes |
| --- | --- |
| [TechnicalSEO.com (Merkle) Schema Generator](https://technicalseo.com/tools/schema-markup-generator/) | Long-standing leader; form-per-type, live preview, validator links. |
| [JSON-LD.com Generator](https://jsonld.com/json-ld-generator/) | v2.0 (Apr 2026) — 17 types, live required-field validation, one-click Rich Results Test / schema.org validator. |
| [Backlinko Schema Markup Generator](https://backlinko.com/tools/schema-markup-generator) | 15 types, no signup, no limits, copy-paste. |
| [Axiolo Free Schema Generator](https://www.axiolo.com/tools/schema-generator) | FAQ/Article/Breadcrumb/WebPage, no account, instant copy. |
| [Web Code Tools Structured Data Generator](https://webcode.tools/structured-data-generator) | Broad type list, "bug-free" JSON-LD copy-paste. |

## Gap analysis (fit to gizza's pure-Rust + browser-local model)

**Closed this run**

- **Type coverage.** Competitors ship 15–17 types; we shipped 10. Added `Review`,
  `VideoObject`, `HowTo`, `JobPosting`, `Course`, `SoftwareApplication` → **16 types**, matching the
  leaders' breadth. Added `reviewRating`→`Rating` and `hiringOrganization`→`Organization` to the
  nested-`@type` inference for the new types.
- **Ready-to-paste script tag.** `wrap_script` already emits the full
  `<script type="application/ld+json">…</script>` block — parity with competitors' "copy code" output.
- **Numeric / array typing.** Differentiator vs. competitors that emit everything as strings: list
  fields (`keywords`, `sameAs`, `ingredients`) become JSON arrays and numeric fields (`price`,
  `ratingValue`, …) become JSON numbers automatically.
- **Nested objects via a flat syntax.** `author.name = …`, `offers.price = …`,
  `address.streetAddress = …` build correctly-typed nested objects — most form-based competitors need
  a separate sub-form per nested object; our dotted syntax is faster for power users and works
  identically across chat/CLI/page.
- **FAQ relevance note.** Google deprecated FAQ rich-result snippets (May 2026), but `FAQPage` markup
  is still valid and useful for search/AI understanding — documented in the page copy so users aren't
  misled, while keeping the type.

**Intentionally NOT built (out of model or low value)**

- **Live "Rich Results Test" / schema.org validator buttons.** These deep-link to Google/schema.org
  external validators. gizza pages are static + offline-first; an outbound validator link is a copy
  concern, not a capability — and we never replicate a competitor's branding. The output is
  schema.org-valid by construction. Skipped.
- **Per-type guided forms with field-level required-property validation.** Competitors render a
  bespoke form per type. gizza's single descriptor → one schema → one page form is the architecture;
  a per-type dynamic form is a site-framework change, not a tool change. The free-form `fields` box
  plus inferred nesting covers the same data with less UI. Deferred.
- **Image/logo upload for `image`/`logo` properties.** Those properties take a URL; users paste a URL
  (`logo = https://…`). A binary upload would need a media-input surface this pure tool doesn't have.

## Result

Built + verified on all three surfaces; broadened to 16 schema types to match competitor breadth, kept
the differentiating auto-typing + dotted-nesting syntax. No competitor copy, branding, or trademarks
were reproduced.
