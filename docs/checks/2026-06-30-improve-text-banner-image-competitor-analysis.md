# text-banner-image — competitor analysis & surface checks (2026-06-30)

**Tool:** `text-banner-image` — render stylized headline text to a PNG banner with gradient/accent background, optional shadow/outline, colours, alignment, and auto-sizing.

## Verification snapshot

Verified on 2026-06-30 (CARGO_BUILD_JOBS=1).

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/text-banner-image && cargo test --workspace` | ✅ 16 passed (15 core + 1 schema drift guard) |
| Wafer block | `cd blocks/text-banner-image && wafer build` | ✅ OK gizza-ai/text-banner-image v0.1.0 (984.7 KiB) |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ 1/1 pass (basic) |
| Web build | `wasm-pack build blocks/text-banner-image/web --target web --release --out-dir pkg` | Not applicable: PNG/image media-output tool has no page/web wrapper in current gizza pattern |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered landing/index (290 tools); no standalone page for this media-output tool |
| CLI | `gizza tool text-banner-image ...` | ✅ wrote `banner.png` and reported expected 640x240 PNG summary |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-text-banner-image.spec.ts` | Not applicable: image media-output tool has no page surface |

## Competitor scan

Representative tools and feature patterns:

1. **Canva / Adobe Express banner makers** — rich templates, fonts, gradients, shadows, and export to PNG.
2. **Bannerbear / image generation APIs** — parameterized text-to-image templates for social cards and marketing automation.
3. **Kapwing / Fotor text banner tools** — browser editors for text overlays, colours, outlines, and shadows.
4. **Pillow/ImageMagick command-line workflows** — scriptable text rendering and PNG output, but require local install and syntax knowledge.
5. **SVG/HTML screenshot pipelines** — flexible styling, but heavier runtime and browser dependencies.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| PNG banner output | Common banner/image tools | ✅ returns an `image/png` media envelope |
| Width/height controls | Common | ✅ bounded `width` and `height` params |
| Custom headline text | Core feature | ✅ required `text`, supports hard line breaks and word wrap |
| Colours / gradient accents | Common design tools | ✅ base background, accent tint/stripe/underline, text colour |
| Alignment | Common | ✅ left, center, right |
| Font sizing | Common | ✅ fixed size or auto-fit with shrink-to-fit |
| Shadow / outline | Common | ✅ optional drop shadow and outline |
| Arbitrary font upload/template libraries | Advanced editors | Out of scope: bundled Liberation Sans Bold for deterministic local rendering |
| Browser visual editor / page preview | Web editors | Out of scope for current gizza image-output page pattern |

## Notes

The implementation is pure Rust (`fontdue` + `image`) with an embedded Liberation Sans Bold font, avoiding system-font and browser/screenshot dependencies. It targets deterministic, local banner generation for quick social cards, documentation headers, and mock marketing assets rather than full template editing.
