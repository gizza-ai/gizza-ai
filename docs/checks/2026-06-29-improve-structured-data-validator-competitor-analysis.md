# structured-data-validator — competitor analysis & surface checks (2026-06-29)

**Tool:** `structured-data-validator` — extract and validate JSON-LD, microdata, and RDFa structured data from pasted HTML.

## Verification snapshot

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/structured-data-validator && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 15 tests passed (core + descriptor drift guard) |
| Wafer block | `cd blocks/structured-data-validator && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` validated |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ `errors`, `json-output`, and `valid-jsonld` fixtures passed |
| Web build | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/structured-data-validator/web --target web --release --out-dir pkg` | ✅ web/pkg generated |
| Page generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/structured-data-validator/` |
| CLI | `gizza tool structured-data-validator ...` | ✅ reported a valid JSON-LD Article and listed detected properties |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-structured-data-validator.spec.ts` | ✅ 4 tests passed (report, errors, JSON output, deep-link) |

## Competitor scan

Representative tools and references:

1. **Google Rich Results Test** — validates whether a page is eligible for Google rich result features and surfaces item-level errors/warnings.
2. **Schema.org validator** — validates schema.org markup across JSON-LD, microdata, and RDFa and lists detected entities/properties.
3. **Bing Markup Validator / SEO site-audit tools** — report missing required/recommended fields and broken structured data on crawlable pages.
4. **JSON-LD playgrounds / linters** — focus on JSON syntax, `@context`, `@type`, and graph expansion/debugging.
5. **Browser SEO extensions** — extract structured data from the current page and summarize detected schema types.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| Extract JSON-LD from HTML | Common | ✅ script blocks parsed and counted |
| Extract microdata | Schema.org validator-style tools | ✅ `itemscope`/`itemtype`/`itemprop` scanned |
| Extract RDFa | Schema.org validator-style tools | ✅ `typeof`/`property` scanned |
| Invalid JSON-LD detection | JSON-LD validators | ✅ parse errors are error-severity issues |
| Missing `@context` / `@type` checks | Rich results / schema validators | ✅ missing context is an error; missing type is a warning |
| Orphan microdata property checks | Markup validators | ✅ `itemprop` outside `itemscope` is an error |
| Machine-readable output | Developer tooling | ✅ `format=json` returns counts/items/issues/valid |
| Live URL crawling / Google eligibility | Rich Results Test | Out of scope: gizza is local pasted-HTML validation, no crawler or Google-specific policy engine |
| Full schema.org vocabulary and required-field rules | Dedicated validators | Out of scope for this first pass; report focuses on structural and common schema.org mistakes |

## Notes

The implementation is intentionally local and deterministic: paste an HTML fragment, get a quick report, and optionally switch to JSON output for automation. It does not fetch pages or claim search-engine eligibility.
