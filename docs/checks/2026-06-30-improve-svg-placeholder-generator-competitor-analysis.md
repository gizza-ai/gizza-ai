# svg-placeholder-generator — competitor analysis & surface checks (2026-06-30)

**Tool:** `svg-placeholder-generator` — generate a scalable placeholder SVG at a chosen size with optional label, colours, font size, and font family.

## Verification snapshot

Verified on 2026-06-30 (CARGO_BUILD_JOBS=1).

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/svg-placeholder-generator && cargo test --workspace` | ✅ 11 passed (10 core + 1 schema drift guard) |
| Wafer block | `cd blocks/svg-placeholder-generator && wafer build` | ✅ OK gizza-ai/svg-placeholder-generator v0.1.0 (311.3 KiB) |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ 1/1 pass (basic) |
| Web build | `wasm-pack build blocks/svg-placeholder-generator/web --target web --release --out-dir pkg` | Not applicable: image/SVG media output has no page/web wrapper in current gizza generator pattern |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered landing/index (289 tools); no standalone page for this media-output tool |
| CLI | `gizza tool svg-placeholder-generator ...` | ✅ wrote `placeholder.svg` and reported expected 320x180 label/byte summary |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-svg-placeholder-generator.spec.ts` | Not applicable: image/SVG media output has no page surface |

## Competitor scan

Representative tools and feature patterns:

1. **placeholder.com / via.placeholder-style URL generators** — quick dimension-labelled placeholders, often with custom background/text colours.
2. **DummyImage / Placehold.co** — text, size, foreground/background colours, and embeddable URLs for prototypes.
3. **Lorem Picsum / placekitten-style services** — image placeholders with remote photos, useful for demos but not deterministic/local SVG.
4. **Design-system placeholder components** — local rectangle/SVG skeletons sized for layout mocks.
5. **SVG placeholder snippets in build tools** — static inline SVG/data URI placeholders for CSS, docs, and tests.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| Width and height controls | Common across placeholder services | ✅ integer width/height, clamped to 1..4096 |
| Default dimension label | Common (`600×400`) | ✅ automatic dimension label when text is empty |
| Custom label text | Common | ✅ `text` parameter, XML-escaped |
| Background colour | Common | ✅ CSS hex #rgb/#rgba/#rrggbb/#rrggbbaa accepted |
| Text colour | Common | ✅ explicit text colour or automatic readable dark/white contrast |
| Font size and family | Some generators | ✅ explicit font size or auto-fit; configurable CSS font-family |
| Remote image/photo placeholders | Photo placeholder services | Out of scope: local deterministic SVG only |
| URL endpoint hosting | Placeholder SaaS | Out of scope: gizza runs locally and returns a media envelope |
| Browser page preview | Some web generators | Not implemented because current gizza page generator does not render image-bytes/SVG-output tools |

## Notes

The implementation hand-builds compact SVG markup, so it avoids image dependencies, is deterministic, and runs in the chat/CLI Wafer block. All label/font strings are XML-escaped, and colour alpha is accepted for user convenience but dropped because the placeholder rectangle is opaque.
