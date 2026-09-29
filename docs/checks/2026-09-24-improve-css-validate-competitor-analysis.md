# css-validate — competitor analysis (2026-09-24)

Backlog row: `css-validate` — "Validate CSS syntax and flag unknown properties and malformed rules."
(type hint `pure`; the row suggested the `lightningcss` crate — see *Engine decision* below.)

All notes below are **paraphrased observations** of publicly documented behaviour. No competitor
copy, branding, or trademarked wording is reproduced or reused anywhere in this block.

## Dup check (why this is a distinct tool)

- `blocks/format-css` — pretty-printer. Its doc string is explicit that "values are never rewritten
  (lossless)"; `format()` returns a single `Result<String, String>` and only errors on empty input.
  It reports **no diagnostics** and has no property knowledge.
- `blocks/css-autoprefixer`, `blocks/js-css-minifier`, `blocks/css-select-extract`,
  `blocks/css-color-converter`, `blocks/css-px-to-rem`, `blocks/css-gradient-generator`,
  `blocks/css-reset-generator` — transform/generate CSS, none validate it.
- `blocks/html-validate` — same *shape* (multi-issue report with line:column, `report`/`json`),
  different language. Its FAQ explicitly says it "does not verify … CSS".

No existing block reports CSS diagnostics → build it.

## Competitors scanned

1. **CSS Portal — CSS Validator** (cssportal.com/css-validator/)
2. **W3C CSS Validation Service** (jigsaw.w3.org/css-validator/) — the reference implementation
3. **TestMu AI — Free Online CSS Validator** (testmuai.com/free-online-tools/css-validator/)

(codeshack.io/css-validator/ was the 4th hit but returns HTTP 403 to non-browser clients, so it was
replaced by the W3C service rather than running the scan with fewer than three.)

### Table-stakes capabilities observed

| Capability | Seen in | Fit | Where it landed |
| --- | --- | --- | --- |
| Paste CSS into a text area, validate on demand | all 3 | in-model | `css` (required, multiline textarea) |
| Multi-issue list, each with a **line number** | all 3 | in-model | every issue carries 1-based line **and column** |
| Error vs warning severity split | all 3 | in-model | `severity` field per issue; `valid` = no errors |
| Filter results (all / errors only / warnings only) | CSS Portal (dropdown) | in-model | `severity` enum `all\|error\|warning` |
| Unknown-property detection, separated from bad values | CSS Portal, TestMu | in-model | curated standard-property list + `unknown_properties` enum |
| Configurable handling of vendor extensions | W3C ("vendor extensions: default / warnings / errors") | in-model | `vendor_prefixes` enum `ignore\|warn\|error` |
| Unbalanced braces / missing `}` | all 3 | in-model | unclosed-block + stray-`}` errors |
| Declaration with no colon, empty value | TestMu, CSS Portal | in-model | dedicated errors |
| Block with no selector; stray text after the last rule | TestMu | in-model | dedicated errors |
| Missing-semicolon detection | CSS Portal, TestMu | in-model | "possible missing `;`" warning (a value carrying a second `prop: value` pair) |
| Malformed selectors (bad combinator, empty comma part) | CSS Portal, TestMu | in-model | selector checks (trailing combinator, empty part, unclosed `[`, nameless pseudo) |
| At-rule checks (`@media`, `@supports`, `@keyframes`, `@layer`, `@container`) | CSS Portal | in-model | known-at-rule list + prelude/block-shape rules |
| `calc()` syntax + operator validity | CSS Portal | in-model | empty/unbalanced/trailing-operator/un-spaced `+ -` errors |
| Custom properties: declarations tracked, usage tracked | CSS Portal | in-model | declared/referenced stats + undeclared-`var()` warning |
| Summary stats (rule count, unique properties, most-frequent properties, variables, at-rules) | CSS Portal | in-model | `stats` boolean (default on), rendered in both formats |
| Machine-readable output | none of the 3 ship it | in-model, **our differentiator** | `format=json` |
| Example / "load sample" button | CSS Portal ("copy example") | in-model | four `[[example]]` preset chips |
| Runs locally, nothing uploaded | CSS Portal claims browser-only; W3C + TestMu are server-side | in-model | genuinely local wasm — stated on the page |

### Out-of-model (listed, deliberately NOT built)

- **Validate by URL / by file upload** (W3C, TestMu). This block is a pure paste-in tool with no
  network capability; fetching a stylesheet (and, for the W3C flow, extracting `<style>`/`<link>`
  CSS out of an HTML page) is a different input surface. `blocks/css-select-extract` covers the
  fetch-a-page direction.
- **Per-property value grammars** — full keyword/length/color/range checking for every property
  (CSS Portal, W3C), e.g. rejecting `color: notacolor` or `z-index: 3px`. That needs the complete
  CSS value-definition syntax database per property level; honest generic value checks (hex colors,
  `calc()`, `!important` spelling, balanced parens/strings, empty values) are implemented instead,
  and the page says so.
- **Shorthand expansion validation** (`border`, `transition`, `animation`, `background` component
  order — CSS Portal). Same reason: needs the per-property grammar DB.
- **CSS profile / level selection** (CSS 1 / 2 / 2.1 / 3, SVG, mobile/TV profiles) and **media
  type** selection (W3C). Requires a per-level property/at-rule matrix; we validate against one
  modern standard-property set.
- **Info-severity advisory tier** (CSS Portal's third level). Deliberately two severities so
  `valid` has one unambiguous meaning; advisory findings are reported as warnings.
- **Fix suggestions / auto-correct.** Reported, not rewritten — `format-css` and `js-css-minifier`
  own rewriting.

## Engine decision — hand-rolled scanner, not `lightningcss`

The backlog row suggested `lightningcss`. Rejected on capability, not just wasm risk:

- `lightningcss` is a *compiler* — it bails on the **first** parse error, so it cannot produce the
  multi-issue line/column list every competitor ships;
- it treats an unknown property as an opaque custom property and passes it through, so it cannot
  flag `colr` at all — the row's headline requirement;
- it would pull a large dependency tree (`cssparser`, `parcel_selectors`, …) that must
  *instantiate* under `wasm32-wasip1`, not merely compile.

A dependency-free scanner is the proven pattern in this repo for exactly this shape
(`blocks/html-validate`'s forgiving HTML scanner, `blocks/format-css`'s CSS tokenizer), gives every
issue a line *and* column, and keeps the block wasm small. Zero new dependencies were added.

## Descriptor as built

| Param | Type | Default | Notes |
| --- | --- | --- | --- |
| `css` | string, required | — | multiline textarea on the page |
| `format` | enum `report`/`json` | `report` | JSON is the differentiator vs all 3 competitors |
| `severity` | enum `all`/`error`/`warning` | `all` | mirrors CSS Portal's results filter |
| `unknown_properties` | enum `warn`/`error`/`ignore` | `warn` | default is `warn`, not `error`, because the property list is curated — a brand-new property must not make a valid stylesheet report as invalid. Users who want the W3C's strict behaviour set `error`. |
| `vendor_prefixes` | enum `ignore`/`warn`/`error` | `ignore` | mirrors the W3C vendor-extensions control |
| `stats` | boolean | `true` | the CSS Portal summary block |

## Verification performed

`cargo test --workspace` (core + drift-guard), `scripts/build-block-wasm.sh css-validate`,
`wasm-pack build … --target web --release`, `sync-tool-manifest.py`, generator render, `gizza tool
css-validate …` incl. an exact-output case and the page's generated CLI example copy-pasted
verbatim, Playwright `tool-page-css-validate.spec.ts` (real output + a `?param=` deep link + every
enum choice + a non-default checkbox state), and `check-tool-hygiene.py css-validate` exit 0.
