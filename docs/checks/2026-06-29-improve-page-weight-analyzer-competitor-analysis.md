# Page Weight Analyzer competitor analysis (2026-06-29)

Tool: `page-weight-analyzer`

## Competitors reviewed

1. DebugBear Page Size Checker
   - URL-based page-size checker with resource contribution breakdown by HTML/CSS/JS/images/fonts.
   - Strong real-network measurement, but requires fetching a public URL.
2. Swetrix Page Size Checker
   - Checks URL page size, linked resource count, transfer size, largest assets, and budget warnings.
   - Useful monitoring-style output; URL/network focused.
3. SEO Site Checkup Render Blocking Resources Test
   - Detects JavaScript/CSS resources that block rendering and reports pass/fail SEO-style guidance.
   - Narrower render-blocking focus; not a local pasted-HTML analyzer.
4. Rank SEO Tools / similar page-size checkers
   - Fetch a URL and measure downloaded resources, HTTP requests, and page weight.
   - Good for live public pages, less useful for local/private HTML snippets.
5. AIO Copilot / performance-budget analyzers
   - Combine page-speed guidance, critical rendering path explanations, and page-weight budget targets.
   - Often cloud/URL-oriented and less scriptable.

## In-model gaps and actions taken

- Private pasted-HTML workflow: implemented static analysis on pasted HTML only; no network fetches are made, so private/local HTML can be audited.
- Resource counts: implemented counts for scripts, stylesheets, images, iframes, audio/video, and resource hints.
- Blocking classification: detects parser-blocking classic external scripts, inline synchronous scripts, async/defer/module scripts, and render-blocking stylesheets while excluding print-only/disabled sheets.
- Inline code size: measures inline JavaScript and CSS byte sizes exactly.
- Requests/weight estimate: computes a lower-bound request count and a rough transfer-weight estimate using measured HTML bytes plus documented per-resource medians.
- Resource listing: optional URL listing groups external script, style, image, font-preload, iframe, and media URLs.
- Scriptable output: supports human-readable report and structured JSON output.
- Page copy: documents estimate limitations, render-blocking definitions, examples, privacy, and JSON output.

## Out-of-model or intentionally not implemented

- Live URL crawling and real transfer sizes: would require network fetches and redirects/CORS handling; this tool intentionally stays local/private.
- Lighthouse/Core Web Vitals scoring: out of scope because no browser runtime, layout, CPU, or network timing is measured.
- CSS/JS-discovered resources: static pasted HTML cannot see runtime fetches, CSS `url()` references, or dynamically imported chunks; the report labels request counts as a lower bound.
- Waterfall visualization: useful competitor feature, but requires measured request timing rather than static markup analysis.

## Verification snapshot

- `cargo test --workspace` from `blocks/page-weight-analyzer`: passed.
- `wafer build` from `blocks/page-weight-analyzer`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/page-weight-analyzer/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered `tools/page-weight-analyzer/`.
- `gizza tool page-weight-analyzer html='...' output=report list_resources=true`: passed.
- `cd tests && xvfb-run npx playwright test tool-page-page-weight-analyzer.spec.ts --timeout=120000 --reporter=line`: passed.
