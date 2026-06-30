# podcast-feed-parser competitor analysis (2026-06-29)

## Tool surface verified

- Chat/CLI: accepts pasted podcast RSS/XML or Atom feed text; returns JSON channel metadata and episode entries with title, publish date, duration, audio URL/type/length, GUID, link, season/episode, explicit flag, and optional descriptions.
- Page: `/tools/podcast-feed-parser/` with multiline feed input plus limit/order/include-description controls.
- Privacy: parsing is local and deterministic; no feed XML is uploaded and no network fetch is performed.

## Competitors reviewed

1. **Podnews / podcast validator-style tools** — useful for feed health and directory compliance, but focused on validation rather than extracting a compact episode JSON list.
2. **Cast Feed Validator / Podbase-like validators** — show feed issues and metadata, but not a copy-friendly structured episode export.
3. **Online RSS readers/parsers** — can display item titles and links, but generally ignore podcast-specific iTunes duration/enclosure metadata.
4. **Feedparser libraries (Python/Node)** — robust and scriptable, but require a coding environment.
5. **Podcast hosting dashboards** — rich episode metadata, but tied to an account/host and not for arbitrary pasted XML.

## Fit-to-model gaps and decisions

- Built in-model: RSS 2.0 podcast feeds, iTunes namespace fields, Atom enclosure links, duration normalisation, publish-date normalisation, limit/order options, optional descriptions, JSON output, page/CLI/chat parity.
- Not built: network fetching by URL, full feed validation scoring, OPML subscription lists, transcript parsing, media probing, or directory-specific compliance checks. Those are separate network/media/validator tools.
- Copy/branding: no competitor copy or proprietary output format was copied.

## Verification snapshot

- `cargo test --workspace` from `blocks/podcast-feed-parser/`
- `wafer build` from `blocks/podcast-feed-parser/`
- `wasm-pack build blocks/podcast-feed-parser/web --target web --release --out-dir pkg`
- `cargo install --path cli`
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`
- `gizza tool podcast-feed-parser ... order=oldest limit=1`
- `cd tests && xvfb-run npx playwright test tool-page-podcast-feed-parser.spec.ts`
- `npm run test`
