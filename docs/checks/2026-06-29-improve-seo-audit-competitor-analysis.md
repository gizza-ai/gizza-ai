# SEO Audit competitor analysis (2026-06-29)

Tool: `seo-audit`

## Competitors reviewed

1. Google Lighthouse / PageSpeed Insights SEO audit
   - Audits a rendered page for title, meta description, crawlability, valid hreflang, structured data, and tap targets.
   - Authoritative, but requires a live URL and a headless browser run; not a private pasted-HTML check.
2. Ahrefs / Semrush On-Page SEO Checker
   - Crawl a public URL and report title/description length, heading structure, image alt coverage, canonical, and Open Graph issues.
   - Strong site-wide auditing, but cloud/URL based and account-gated.
3. SEO Site Checkup / Seobility single-page analyzers
   - Per-URL checklist covering meta tags, headings, viewport, robots, and social tags with pass/warn/fail markers.
   - Useful checklist UX; fetch a URL rather than auditing local markup.
4. Nu Html Checker + meta-tag preview tools (metatags.io, opengraph.xyz)
   - Validate markup and preview Open Graph / Twitter Card social cards.
   - Focused on validity and preview, not a scored on-page SEO roll-up.
5. Browser SEO extensions (Detailed, SEO Minion, META SEO inspector)
   - Inspect the current tab's title, description, headings, canonical, hreflang, and structured data.
   - Convenient in-browser, but tied to the active live tab rather than arbitrary pasted HTML.

## In-model gaps and actions taken

- Private pasted-HTML workflow: implemented static analysis on pasted HTML only; no network fetch, so private/staging/local HTML can be audited without uploading.
- Scored roll-up: implemented a 0–100 score with an A–F grade and a passed/warnings/failed summary so results are skimmable.
- Title and meta description: implemented presence plus recommended length bounds (title 10–60, description 50–160) with short/long warnings.
- Heading checks: implemented single-`<h1>` enforcement and skipped-heading-level detection (e.g. h2 → h4) over the document heading sequence.
- Image alt coverage: implemented per-`<img>` non-empty alt detection (quoted, single-quoted, and bare attributes).
- Crawl/index directives: implemented canonical-link, robots-meta (noindex/nofollow), meta-refresh redirect, and hreflang/x-default checks.
- Mobile + encoding basics: implemented viewport (width=device-width) and charset declaration checks.
- Social + rich results: implemented Open Graph (og:title/description/image), Twitter Card, and structured-data (JSON-LD / microdata) detection.
- Extras: implemented `<html lang>`, favicon link, descriptive-anchor-text, and deprecated-element checks to broaden coverage beyond the headline meta tags.

## Out-of-model or intentionally not implemented

- Live URL crawling: would require network fetches, redirects, and CORS handling; the tool intentionally stays local/private and audits only pasted markup.
- Rendered/runtime SEO: JS-injected tags, client-rendered headings, and Core Web Vitals need a browser runtime and timing, which static markup analysis cannot observe.
- Site-wide auditing: cross-page duplicate-title/description detection, internal-link graphs, and sitemap/robots.txt coverage are out of scope for a single pasted page.
- Keyword/content scoring: keyword density and readability scoring are subjective and omitted to keep the audit deterministic and language-agnostic.

## Verification snapshot

- `cargo test --workspace` from `blocks/seo-audit`: passed.
- `wafer build` from `blocks/seo-audit`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/seo-audit/web --target web --release --out-dir pkg`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered `tools/seo-audit/`.
- `cargo install --path cli`: passed.
- `gizza tool seo-audit html='...'`: passed.
- `cd tests && xvfb-run npx playwright test tool-page-seo-audit.spec.ts`: passed.
