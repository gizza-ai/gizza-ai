# Multilingual Tool Pages and `translate-tool` Implementation Plan

> **For agentic workers:** Implement this plan task-by-task. Keep each task in its own
> reviewable commit where practical. The plan creates multilingual page infrastructure and a
> repo-local `/translate-tool` skill; it does not translate or change tool computation.

**Goal:** Add complete, crawlable language variants of gizza's static tool pages and create an
AI-driven `/translate-tool <slug> <locale>` workflow that produces, validates, tests, and opens a
review-only PR for one tool translation at a time.

**Architecture:** English remains the source of truth in `blocks/<slug>/page/{meta.toml,content.md}`.
Human-facing locale overlays live in `blocks/<slug>/page/i18n/<locale>/`; they may override only
copy, never tool identity, schema, parameter names, enum values, WASM exports, or behavior. A typed
generator-wide message catalog localizes shared UI, category, Markdown, Open Graph, and conversion
page copy. The generator renders English at `/tools/...` and language variants at
`/<url-locale>/tools/...`, using the same English tool assets/WASM rather than copying binaries for
every locale. Every published language page is complete, self-canonical, and connected to its
available variants by reciprocal `hreflang` links.

**Translation strategy:** The first version uses the active coding agent/LLM for contextual
translation and localization. Deterministic scripts enforce structure, source freshness, protected
technical tokens, and page completeness. A fresh read-only AI review pass checks accuracy and
fluency; native-speaker review is recorded when it happens but is not falsely claimed. A dedicated
translation API (DeepL/Google/etc.) is a deferred provider option, not a v1 dependency.

**Tech stack:** Rust (`serde`, `toml`, `maud`, existing page generator), Markdown, Python 3 standard
library validation scripts, vanilla ES modules, Playwright, repo-local Claude/Codex skill files.

## Product and SEO decisions

- Keep the existing English URLs unchanged: `/tools/<slug>/`.
- Use BCP 47 language tags for metadata/hreflang (`pt-BR`) and an explicit lowercase URL prefix
  from the locale registry (`pt-br`).
- Keep tool and pair slugs language-neutral in v1. Do not translate URL slugs.
- Give every language page a self-referencing canonical. Never canonicalize a translated page to
  English.
- Emit reciprocal `hreflang` entries for every available variant, including the current page, plus
  `x-default` pointing at English.
- Never select content using IP address or `Accept-Language`, and never force a language redirect.
  A visible language selector links directly between equivalent pages.
- Render a locale page only when its per-tool overlay and the locale's shared catalog are complete.
  Do not fill missing translated fields with visible English fallback.
- A locale landing page and category hub list only tools that are complete in that locale. Related
  tool and conversion links likewise point only to localized pages that exist.
- Preserve code, inline code, CLI commands, URLs, query parameter names, enum values, file format
  names where customary, and machine-parsed numeric examples. Translate explanations around them.
- Render localized Open Graph images. The locale registry must declare a font with adequate glyph
  coverage before adding non-Latin scripts.
- Support `dir = "ltr" | "rtl"` in the model from the start, even though the initial Spanish pilot
  is LTR.

## Proposed source layout

```text
tools/generator/i18n/
├── locales.toml                  # locale registry: tag, URL prefix, autonym, dir, OG locale
├── en.toml                       # complete source catalog for generator-owned strings
└── es.toml                       # complete shared Spanish catalog (created during pilot)

blocks/url-encode/page/
├── meta.toml                     # unchanged English source + non-translatable behavior
├── content.md                    # unchanged English source
└── i18n/
    └── es/
        ├── meta.toml             # translated fields only + source hash/review metadata
        └── content.md            # fully translated prose

.claude/skills/translate-tool/
├── SKILL.md                      # concise trigger + ordered workflow
├── reference.md                  # schemas, commands, reviewer prompt, PR template
└── agents/openai.yaml            # UI metadata when initialized by skill-creator tooling
```

The translated metadata overlay should be intentionally unable to represent behavior fields:

```toml
schema_version = 1
locale = "es"
source_hash = "sha256:<hash of the translatable English projection>"
review = "ai"                    # "ai" or "native"; never infer native review

title = "Codificador y decodificador de URL"
description = "..."
h1 = "Codificador/decodificador de URL"
hero_subtitle = "..."
output_label = "Resultado"
tags = ["codificar url", "decodificar url", "codificación porcentual"]

[[input]]
name = "text"                    # stable source key; never translated
label = "Texto o URL"
placeholder = "name=John Doe&city=São Paulo"

[[input]]
name = "mode"
label = "Modo"

[input.labels]
encode = "Codificar"             # canonical enum key stays unchanged
decode = "Decodificar"

[[example]]
index = 0                         # source example index; only its visible label is translated
label = "Codificar una URL"
```

The exact TOML representation may be adjusted while implementing Task 2, but the invariants above
must remain: stable keys, translated visible fields only, strict completeness, and no duplicated
behavior/schema.

## Global constraints

- Do not edit `core/`, block descriptors, `manifest.json`, WASM exports, parameter names, or enum
  values as part of translation.
- Do not translate code blocks, inline code, URLs, CLI commands, query keys, MIME types, file
  extensions, or data-format tokens unless an explicit glossary rule permits the display form.
- Do not copy localized competitor text. A target-language search may inform terminology/query
  wording only; all output must be original.
- Do not add per-tool slug branches to shared runtime JS. Localized behavior must be driven by
  locale data and stable metadata.
- Keep English rendering and existing `/tools/...` paths backward compatible.
- Do not duplicate JS/WASM bundles under every locale. Locale HTML must use an explicit shared
  asset base rooted at the English tool directory.
- Do not publish partially translated pages. Missing required translation data is a build error in
  strict/CI mode and causes the page to be omitted in non-strict local inspection mode.
- Do not claim native-speaker review unless a named human review actually happened.
- Do not add a paid translation service or API key requirement in v1.
- Keep generated `pkg/` output out of translation source-of-truth decisions; source overlays and
  catalogs are committed, generated pages are reproducible artifacts.

---

## Task 1: Add the locale registry and typed shared message catalog

**Files:**

- Create: `tools/generator/src/i18n.rs`
- Create: `tools/generator/i18n/locales.toml`
- Create: `tools/generator/i18n/en.toml`
- Modify: `tools/generator/src/main.rs`
- Modify: `tools/generator/Cargo.toml` only if a genuinely necessary dependency is identified

**Interfaces:**

- `LocaleSpec`: `tag`, `url_prefix`, `autonym`, `dir`, `og_locale`, optional font selection.
- `Messages`: a typed, required-field catalog for all generator-owned visible strings.
- `LocaleContext`: selected `LocaleSpec` + `Messages`, passed explicitly to renderers.
- Locale URL helper:
  - English `localized_path("/tools/x/") -> "/tools/x/"`
  - Spanish `localized_path("/tools/x/") -> "/es/tools/x/"`

- [ ] Write failing Rust tests for registry parsing, unique tags/prefixes, default English, BCP 47
      tag validation, `ltr|rtl`, and English/Spanish URL generation.
- [ ] Add a test proving unknown and missing shared catalog keys fail loudly. Prefer typed structs
      with `#[serde(deny_unknown_fields)]` over an unvalidated string map.
- [ ] Inventory and assign stable message keys for visible strings in `template.rs`, `markdown.rs`,
      `index.rs`, `categories.rs`, `pairs.rs`, `formats.rs`, `og.rs`, `site.rs`, and runtime JS.
- [ ] Create a complete English catalog whose rendered wording matches current English behavior.
- [ ] Keep classification-only category tags/slug rules language-neutral; localize only category
      display titles and blurbs.
- [ ] Run `cargo test --manifest-path tools/generator/Cargo.toml` and commit.

**Acceptance:** A locale cannot be registered without a complete typed shared catalog, and English
paths/catalog output remain unchanged.

---

## Task 2: Define and load strict per-tool translation overlays

**Files:**

- Create: `tools/generator/src/translation.rs`
- Modify: `tools/generator/src/meta.rs`
- Modify: `tools/generator/src/main.rs`
- Add test fixtures under: `tools/generator/tests/fixtures/i18n/` (or module-local temp fixtures)

**Interfaces:**

- `ToolTranslation` contains only the human-facing fields shown in the proposed TOML.
- `LocalizedToolMeta` is produced by merging `ToolMeta + ToolTranslation`; renderers consume this
  view while runtime/schema code continues to consume stable source identifiers.
- Inputs map by stable `name`; enum display labels map by canonical enum value; example labels map
  by validated source index.

- [ ] Write failing tests for a valid overlay merge.
- [ ] Write rejection tests for attempts to override `slug`, `wasm`, `export`, `runtime`, `format`,
      input names/order, source type, enum values, defaults, or example params.
- [ ] Require translated `title`, `description`, `h1`, `hero_subtitle`, `output_label`, tags, every
      visible input label/placeholder, enum display label, and example label that exists in source.
- [ ] Allow an empty translated placeholder only when the source placeholder is also empty and the
      control does not require one.
- [ ] Validate overlay locale matches its directory and a registered locale.
- [ ] Define optional per-tool custom UI strings as data (`custom_strings`) rather than translated
      copies of `custom.js`. Audit the nine current custom scripts and identify any visible strings
      that must move behind stable keys before their tools can be localized.
- [ ] Make malformed/incomplete overlays fatal in strict mode and omitted-with-warning in explicit
      non-strict development mode.
- [ ] Run all generator tests and commit.

**Acceptance:** The type system and parser make it impossible for a translation overlay to change
tool behavior or silently omit user-visible source fields.

---

## Task 3: Make URLs, canonical metadata, and alternate clusters locale-aware

**Files:**

- Modify: `tools/generator/src/site.rs`
- Modify: `tools/generator/src/template.rs`
- Modify: `tools/generator/src/pairs.rs`
- Modify: `tools/generator/src/index.rs`
- Modify: `tools/generator/src/markdown.rs`
- Modify: `tools/generator/src/descriptor.rs`
- Modify: `tools/generator/src/og.rs`

**Interfaces:**

- `PageIdentity`: page kind/stable key + current locale + path + available locale variants.
- `AlternateLink`: `hreflang`, absolute/site-relative `href`, and `x-default` handling.
- `SiteConfig::abs_localized(locale, path)` and `url_or_rel_localized(locale, path)`.
- English remains the default locale and `x-default` destination.

- [ ] Write tests for self-canonical English and Spanish pages.
- [ ] Write tests for reciprocal alternates containing English, Spanish, the current page, and
      `x-default`; ensure unavailable translations are absent.
- [ ] Render `<html lang="..." dir="...">` from locale data on tool, landing, hub, and pair pages.
- [ ] Render localized `og:url`, localized OG image paths, `og:locale`, and available
      `og:locale:alternate` values.
- [ ] Localize visible and JSON-LD breadcrumbs while keeping URLs locale-correct.
- [ ] Keep `tool.json` as the language-neutral machine descriptor at the English asset path in v1;
      localized HTML may link to it directly. Render localized `index.md` separately.
- [ ] Omit English-only Atom feed autodiscovery from non-English landing pages until localized feeds
      exist.
- [ ] Extend `SiteConfig` with optional per-locale header/footer fragments. In generic builds,
      synthesize translated generic chrome from the catalog. In branded strict builds, fail rather
      than render a locale with an undeclared English chrome fallback.
- [ ] Run generator tests and commit.

**Acceptance:** Every localized page identifies itself, its language, and only its real equivalents
correctly; no localized page canonicalizes to English.

---

## Task 4: Extract shared HTML, Markdown, OG, and runtime UI strings

**Files:**

- Modify: `tools/generator/src/template.rs`
- Modify: `tools/generator/src/markdown.rs`
- Modify: `tools/generator/src/index.rs`
- Modify: `tools/generator/src/categories.rs`
- Modify: `tools/generator/src/og.rs`
- Modify: `tools/generator/src/site.rs`
- Modify: `tools/generator/assets/runtime/tool.js`
- Modify: `tools/generator/assets/runtime/tool-ffmpeg.js`
- Modify: `tools/generator/assets/runtime/tool-audio.js`
- Modify: `tools/generator/assets/runtime/header.js`
- Modify tests: `js/*.test.js` and generator unit tests

- [ ] Replace hard-coded UI text such as Add, Download, Reset, Copy, Copied, Processing, related
      tools, popular conversions, CLI headings, breadcrumbs, landing search, empty states, and
      waveform labels with typed catalog values.
- [ ] Bake the small runtime message subset into `window.GIZZA_TOOL.i18n`; do not fetch translation
      files at runtime.
- [ ] Pass localized labels into the waveform and ffmpeg paths rather than embedding English in
      their modules.
- [ ] Replace `header.js`'s hard-coded `/tools/_index.json` and `/tools/<slug>/` with a baked site
      context containing locale index URL and locale tool-base URL.
- [ ] Localize Markdown twins completely, including generated headings and explanatory bullets.
      Keep code/parameter identifiers canonical.
- [ ] Localize generic header/footer text and Open Graph card straplines.
- [ ] Add English regression assertions so moving strings into catalogs does not accidentally alter
      current wording or behavior.
- [ ] Run generator tests and `npm test`; commit.

**Acceptance:** A localized tool page contains no generator/runtime-owned English UI fallback, and
the English page still behaves as before.

---

## Task 5: Render localized tool pages, indexes, hubs, and language navigation

**Files:**

- Modify: `tools/generator/src/main.rs`
- Modify: `tools/generator/src/template.rs`
- Modify: `tools/generator/src/index.rs`
- Modify: `tools/generator/src/related.rs`
- Modify: `tools/generator/src/og.rs`
- Create or modify: generator tests for output trees/manifests

**Output contract:**

```text
pkg/tools/url-encode/             # existing English HTML + shared JS/WASM/assets
pkg/es/tools/url-encode/          # localized HTML/Markdown/OG; references shared English assets
pkg/es/tools/index.html
pkg/es/tools/_index.json
pkg/es/tools/<category>/
pkg/tools/_locales.json           # all page clusters for sitemap/deployment integration
```

- [ ] Preserve current `--out` semantics for the English tools directory. Derive localized sibling
      roots from its parent (`pkg/tools` -> `pkg/es/tools`). Add `--locale <tag>` as a development
      restriction; the default build renders every registered locale.
- [ ] Add an explicit per-page asset base (`/tools/<slug>/`) so localized pages reuse English
      `tool.js`, custom modules, ffmpeg modules, CSS, wasm-bindgen JS, and WASM.
- [ ] Never copy WASM or per-tool runtime bundles into localized output directories.
- [ ] Render each locale's landing page from only its translated tool metas.
- [ ] Render locale category hubs only when they have at least one translated member.
- [ ] Restrict localized related-tool cards to translated targets and locale-correct URLs.
- [ ] Add a visible, accessible language selector to tool, landing, hub, and later pair pages. Show
      language autonyms and only real equivalent links.
- [ ] Produce locale-aware `_index.json`, `_hubs.json`, `index.md`, and localized OG cards.
- [ ] Produce `_locales.json` with page kind, stable key, and all alternate paths. Include tools,
      landing pages, hubs, and pairs once pair localization lands.
- [ ] Ensure adding a translation regenerates the English page so its reciprocal `hreflang` link is
      present.
- [ ] Run the full generator twice and compare source-controlled output or checksums as appropriate
      to prove deterministic output. Commit.

**Acceptance:** A localized page is functionally identical to English, has complete localized
navigation/content, and adds negligible binary size beyond HTML/Markdown/OG assets.

---

## Task 6: Localize generated conversion-pair pages without pair-by-pair copied logic

**Files:**

- Modify: `tools/generator/src/pairs.rs`
- Modify: `tools/generator/src/formats.rs`
- Modify: `tools/generator/src/main.rs`
- Extend: `tools/generator/i18n/en.toml`
- Extend each locale catalog when pair pages are enabled

- [ ] Separate language-neutral format facts and direction logic from localized format descriptions,
      headings, table labels, warnings, CTA copy, steps, and FAQ templates.
- [ ] Represent pair prose as complete message templates with named placeholders such as `{src}` and
      `{tgt}`. Do not build grammar from English fragments or assume English articles/word order.
- [ ] Validate exact key and placeholder parity between English and every enabled pair catalog.
- [ ] Pass `LocaleContext` through pair title, description, content, sibling links, Markdown, JSON-LD,
      CTA, and OG rendering.
- [ ] Make pair CTA/deep-link URLs open the localized parent page while preserving canonical query
      keys/values.
- [ ] Render localized pair pages only when the parent tool translation and the locale's complete pair
      catalog both exist. Until then, suppress localized “Popular conversions” links rather than
      linking to English pages from localized copy.
- [ ] Add representative tests for audio lossless-to-lossy, lossy-to-lossless, and image alpha/
      animation cases in English and Spanish.
- [ ] Add all localized pair clusters to `_locales.json` and `_pairs.json`; commit.

**Acceptance:** All 50 generated conversion pages can be localized from one locale catalog while
retaining pair-specific technical accuracy and natural target-language grammar.

---

## Task 7: Add deterministic translation validation and freshness tracking

**Files:**

- Create: `scripts/check-translations.py`
- Create: `scripts/check-translations.test.sh`
- Modify: `scripts/check-tool-hygiene.py` only to share/invoke checks when that reduces duplication
- Modify: `.github/workflows/test.yml`
- Modify: `justfile`
- Modify: `docs/TOOLCHAIN-SETUP.md`

**Commands:**

```bash
python3 scripts/check-translations.py --all
python3 scripts/check-translations.py url-encode --locale es
python3 scripts/check-translations.py --source-hash url-encode
just check-translations
```

- [ ] Define the source hash as SHA-256 over a canonical projection of translatable English fields,
      `content.md`, and declared custom string sources. Exclude behavior-only metadata so a WASM
      filename change does not invalidate copy.
- [ ] Keep the Python script as the single hash authority; the Rust generator validates overlay shape
      but does not reimplement a subtly different hash algorithm.
- [ ] Check overlay/source hash freshness, schema version, locale registration, required fields,
      stable input names, enum-label keys, example indices, and forbidden behavior fields.
- [ ] Parse Markdown and verify fenced code blocks, inline code, URLs, HTML element balance, CLI
      commands, query keys, interpolation placeholders, and protected glossary terms are unchanged.
- [ ] Apply localized equivalents of existing page hygiene rules: FAQ accordions/count, non-empty
      required placeholders, generic/no-brand copy, and sensible localized meta lengths.
- [ ] Flag suspicious long unchanged English prose while allowing technical tokens through an
      explicit allowlist. Treat this as a review finding rather than pretending heuristics prove
      fluency.
- [ ] Add fixture-based self-tests covering every failure class and one fully valid translation.
- [ ] Run translation validation in CI before generator tests. A committed stale/incomplete
      translation must fail CI; an English edit to a translated tool therefore requires refreshing
      its existing locale overlays in the same PR.
- [ ] Adjust CI change detection so page/i18n-only PRs still run the generator and the focused i18n
      Playwright spec even though they do not rebuild block WASM.
- [ ] Add a derived status/report mode for coverage; do not commit a second status file that can
      drift from the filesystem.
- [ ] Run script self-tests, hygiene self-tests, generator tests, and commit.

**Acceptance:** Automation can prove structural safety and freshness; subjective fluency remains an
explicit AI/human review responsibility.

---

## Task 8: Add multilingual SEO and functional browser tests

**Files:**

- Create: `tests/tool-page-i18n.spec.ts`
- Modify: `tests/fixtures.ts` only if a locale helper is useful
- Modify: generator unit tests in `template.rs`, `pairs.rs`, `index.rs`, `og.rs`, and `site.rs`
- Modify: `.github/workflows/test.yml`

- [ ] Generate an English + Spanish fixture tool during tests.
- [ ] Assert `/tools/<slug>/` and `/es/tools/<slug>/` both load and compute the same result.
- [ ] Assert the localized page contains translated title, H1, input labels, output label, shared
      buttons, FAQ, related links, and no known English UI sentinel.
- [ ] Assert `html[lang]`, `dir`, self-canonical, complete reciprocal `hreflang`, and `x-default`.
- [ ] Assert the language selector links directly between equivalent URLs.
- [ ] Assert localized internal links stay under the locale prefix.
- [ ] Assert localized pages load their module/WASM from `/tools/<slug>/` and no duplicate localized
      WASM request/output exists.
- [ ] Assert localized `_index.json`, hub pages, OG paths, Markdown links, FAQ JSON-LD, and breadcrumb
      JSON-LD are locale-correct.
- [ ] Add pair-page assertions after Task 6.
- [ ] Run:

```bash
cargo test --manifest-path tools/generator/Cargo.toml
python3 scripts/check-translations.py --all
cargo run --manifest-path tools/generator/Cargo.toml -- .
(cd tests && npx playwright test tool-page-i18n.spec.ts)
npm test
```

- [ ] Commit.

**Acceptance:** Tests cover both SEO identity and real tool execution; translations cannot “pass” by
rendering a static page whose tool is broken.

---

## Task 9: Create the repo-local `/translate-tool` skill

**Files:**

- Create: `.claude/skills/translate-tool/SKILL.md`
- Create: `.claude/skills/translate-tool/reference.md`
- Create: `.claude/skills/translate-tool/agents/openai.yaml` when supported by the initializer
- Modify: `.claude/skills/improve-tool/SKILL.md`
- Modify: `.claude/skills/improve-tool/reference.md` only for the translated-source freshness hook

**Skill trigger/input:** Existing tool slug + BCP 47 locale, for example
`/translate-tool url-encode es`. Optional mode: refresh all existing locales for one changed tool.
If slug or locale is missing, that is the only allowed clarification.

- [ ] Initialize `translate-tool` with the skill-creator initializer rather than hand-building a
      malformed skill folder; then keep `SKILL.md` concise and move schemas/commands into
      `reference.md`.
- [ ] Define the ordered workflow:
  1. Validate slug, locale registry, clean scope, and English hygiene.
  2. Branch `feat/translate-<slug>-<url-locale>` from the correct base.
  3. Read tool behavior/schema plus all source copy so technical claims are understood.
  4. Read the locale glossary and shared catalog; optionally sanity-check the primary native search
     phrase without copying competitor text.
  5. Translate the structured overlay and full Markdown while protecting canonical tokens.
  6. Calculate and record the source hash.
  7. Run deterministic translation checks.
  8. Run a fresh read-only reviewer pass on source + translation. If subagents are unavailable, run
     a clearly reported second isolated review pass; never claim independence that did not happen.
  9. Fix findings, then run generator and focused Playwright/functionality checks.
  10. Commit, push, and open a review-only PR; never merge.
- [ ] State explicitly that the active LLM is the v1 translation provider and no translation API key
      is required.
- [ ] Add a glossary policy: technical terms may be preserved, transliterated, or localized only as
      the locale glossary declares; canonical machine values never change.
- [ ] Add an honesty gate: stop after three failed validation/fix attempts; report the exact failure;
      never publish a partial overlay or claim native review.
- [ ] Add PR sections for locale/query terminology, files translated, protected tokens, validation
      results, AI review findings, native-review status, known limitations, and source hash.
- [ ] Add `improve-tool` integration: when English translatable copy changes for a tool with existing
      locales, refresh those overlays before its PR can pass translation CI.
- [ ] Validate the skill with `quick_validate.py` and forward-test it on at least:
  - a pure text tool with enum/checkbox controls (`url-encode`),
  - a file/ffmpeg tool,
  - a tool with example chips and custom UI strings.
- [ ] Forward tests must operate on disposable branches/worktrees or fixtures and must not open live
      PRs. Review emitted overlays and validation logs, then iterate on the skill.
- [ ] Commit.

**Acceptance:** One command produces a complete, source-fresh, reviewed translation PR without
changing tool behavior or requiring a paid translation service.

---

## Task 10: Create the Spanish locale and run a controlled pilot

**Files:**

- Create: `tools/generator/i18n/es.toml`
- Modify: `tools/generator/i18n/locales.toml`
- Create: selected `blocks/<slug>/page/i18n/es/{meta.toml,content.md}` overlays
- Add/update: any representative test fixture required by Task 8

- [ ] Register `es` with autonym `Español`, URL prefix `es`, `dir = "ltr"`, and `og_locale = "es_ES"`.
- [ ] Translate and independently review the complete shared catalog first.
- [ ] Select 10–20 pilot tools using Search Console impressions/opportunity when available. Ensure the
      set also covers pure text, numeric, file/ffmpeg, enum, checkbox, example-chip, custom UI, and
      FAQ-heavy page shapes.
- [ ] Run `/translate-tool <slug> es` once per pilot tool. Do not use a bulk loop until the individual
      workflow has passed all representative shapes.
- [ ] Obtain native-speaker review for at least a representative sample and every high-impression
      pilot page; record actual status in each overlay/PR.
- [ ] Run the full matrix from Task 8 plus a production-like branded render using a temporary/test
      site config with Spanish chrome.
- [ ] Review generated pages visually at desktop and mobile widths.
- [ ] Do not enable pair links in Spanish until the Spanish pair catalog from Task 6 passes.
- [ ] Commit/publish via review-only PRs following normal repository policy.

**Pilot exit criteria:** zero mixed-language UI defects, no functionality regressions, correct
canonical/hreflang clusters, successful indexing of representative pages, and acceptable native
review quality. Ranking improvement is monitored but is not a blocker for technical correctness.

---

## Task 11: Integrate the private hosted site, sitemap, and deployment

**Scope boundary:** The hosted site, branded fragments, and SEO sitemap script are not present in this
public repository. Implement this task in the private site repository after the public generator PR
is merged and its pin is bumped.

- [ ] Update the private site pin and generator invocation so all registered locales render.
- [ ] Add localized branded header/footer fragments or declare a translated generic fallback; do not
      reuse English navigation on Spanish pages.
- [ ] Teach sitemap generation to consume `pkg/tools/_locales.json` and emit every language URL with
      identical `<xhtml:link rel="alternate" hreflang="...">` clusters, including self and
      `x-default`.
- [ ] Ensure the host serves `/<locale>/tools/...` as static paths with 200 responses and does not
      rewrite them to English.
- [ ] Ensure robots/caching/service-worker rules include locale-prefixed tool paths and shared English
      assets.
- [ ] Verify deployed HTML, not just local output: status, canonical, hreflang reciprocity, language
      selector, structured data, OG image, CSS/JS/WASM requests, and tool execution.
- [ ] Submit/refresh the sitemap in Search Console and monitor indexing/canonical-selection reports.
- [ ] Keep automatic locale redirects disabled. An optional dismissible language suggestion may be a
      later product enhancement.

**Acceptance:** Production serves complete Spanish variants and exposes the same alternate clusters
in HTML and sitemap without duplicating binaries or breaking English URLs.

---

## Task 12: Scale safely after the pilot

- [ ] Use the derived coverage report to track translated/current/stale/missing tools per locale.
- [ ] Select subsequent languages from search demand, product usage, and review capacity rather than
      translating every language simultaneously.
- [ ] Add each locale by translating the shared catalog, validating font/RTL requirements, then
      processing tools in measured batches.
- [ ] Add a separate `/translate-tool-loop <locale>` skill only after `/translate-tool` has proven
      reliable. It should select one untranslated/stale tool at a time, invoke the full workflow, and
      stop on any failed quality gate.
- [ ] Keep native review concentrated on shared catalogs, high-impression pages, new technical
      terminology, and random samples; do not mislabel AI-only pages.
- [ ] Compare indexation, impressions, CTR, engagement, and translation corrections by locale before
      increasing batch size.
- [ ] Add an optional dedicated translation provider behind the same overlay contract only if it
      improves measured cost/quality/throughput. The validator, reviewer, storage format, and PR gate
      remain provider-independent.

## Final verification matrix

Before declaring the multilingual system complete, record real results for:

```bash
cargo test --manifest-path tools/generator/Cargo.toml
python3 scripts/check-translations.py --all
bash scripts/check-translations.test.sh
bash scripts/check-tool-hygiene.test.sh
npm test
cargo run --manifest-path tools/generator/Cargo.toml -- .
(cd tests && npx playwright test tool-page-i18n.spec.ts)
```

Also verify manually on a production-like server:

- English URLs and tool execution are unchanged.
- A translated pure tool and ffmpeg tool execute correctly.
- Localized pages contain no visible English fallback outside protected technical tokens.
- Canonical, `hreflang`, `x-default`, JSON-LD, OG, and language navigation are mutually consistent.
- Localized pages reuse English JS/WASM assets.
- Landing, hub, related-tool, pair, and search links stay within the selected locale when an
  equivalent translation exists.
- Missing translations are absent rather than mixed-language or broken.

## Recommended PR sequence

1. **PR A — i18n foundation:** Tasks 1–5, English catalog extraction, locale routing, shared assets,
   indexes/hubs, and unit tests. No public non-English locale yet.
2. **PR B — generated pages and quality gates:** Tasks 6–8, pair catalogs, validator, CI, and
   Playwright.
3. **PR C — workflow:** Task 9, `/translate-tool` skill and forward-test fixes.
4. **PR D — Spanish pilot:** Task 10, locale catalog plus the first reviewed tool overlays.
5. **Private-site PR:** Task 11, pin bump, localized chrome/routes/sitemap, and deployment checks.
6. **Later locale/batch PRs:** Task 12, one measured locale/tool batch at a time.

This sequence keeps infrastructure, generated-copy refactoring, autonomous workflow behavior, and
actual translated content independently reviewable while preserving the eventual goal: every useful
tool page available in every supported language.
