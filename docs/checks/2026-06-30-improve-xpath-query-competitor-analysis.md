# xpath-query — competitor analysis & surface checks (2026-06-30)

**Tool:** `xpath-query` — evaluate XPath 1.0 expressions against XML/XHTML and return matching node string values, serialized XML, or scalar results. Pure Rust via `sxd-document` + `sxd-xpath`.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + schema tests | `cd blocks/xpath-query && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 12 core tests + 1 drift-guard schema test pass |
| Chat block | `cd blocks/xpath-query && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` validates |
| Page wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/xpath-query/web --target web --release --out-dir pkg` | ✅ pkg built |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/xpath-query/` |
| CLI | `gizza tool xpath-query expression='//book/title' xml=...` and `count(//book)` | ✅ JSON output contains `Rust`, `XML`, and `2` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-xpath-query.spec.ts` | ✅ 4 passed |

## Competitor landscape

Top comparable utilities:

1. **FreeFormatter XPath Tester / XPath Online Testers** — browser XPath expression testing with sample XML, often with syntax highlighting and tree views.
2. **Code Beautify XPath Tester** — paste XML and XPath to see matching results, focused on simple web use.
3. **xpather.com / browser extension XPath tools** — designed for page scraping/debugging rather than local CLI/chat usage.
4. **xmllint / XMLStarlet CLI** — powerful local XPath from the terminal, but requires installation and command syntax.
5. **IDE XML plugins** — good developer UX but tied to an editor.

## Capability diff

| Capability | Competitors | gizza xpath-query |
| --- | --- | --- |
| XPath 1.0 location paths | common | ✅ via sxd-xpath |
| Attribute selection | common | ✅ `//a/@href` |
| Predicates and numeric comparisons | common | ✅ |
| Scalar functions/results | common | ✅ strings, numbers, booleans |
| Node-set document-order results | common | ✅ one line per match |
| Text-value output | common | ✅ default `value` |
| Serialized outer XML output | some | ✅ `output=xml` |
| Local/private browser execution | varies | ✅ wasm page |
| CLI/chat reuse | fewer | ✅ same core through gizza surfaces |
| Forgiving malformed HTML parsing | some web tools | ❌ out of model; requires well-formed XML/XHTML |
| Namespace prefix mapping UI | some advanced tools | ❌ not configured separately yet |

## In-model gaps closed / confirmed

The tool covers the direct XPath testing workflow: expression + XML input, node value extraction, node XML serialization, scalar result handling, page deep links, and regression tests for predicates, attributes, functions, escaping, and empty matches.

## Out-of-model / intentionally not built

- Browser DOM XPath against live web pages is out of scope; this is a local XML/XHTML document evaluator.
- Forgiving HTML5 parsing is not included; malformed HTML should be normalized to XHTML first.
- Custom namespace prefix mapping UI is deferred; the simple stateless surface has expression, XML, and output mode only.

No competitor copy, branding, or trademarks were used.
