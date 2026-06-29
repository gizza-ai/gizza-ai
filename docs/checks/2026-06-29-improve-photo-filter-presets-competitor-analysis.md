# photo-filter-presets competitor analysis (2026-06-29)

## Tool surface verified

- Chat/CLI block: image input via `url`/`ref`, preset enum (`sepia`, `vintage`, `warm`, `cool`, `noir`, `grayscale`, `vivid`, `invert`, `fade`), output image envelope.
- Page: `/tools/photo-filter-presets/`, browser-local ffmpeg runtime, image upload, preset field, image output.
- Privacy: all image processing is local; no server upload.

## Competitors reviewed

1. **Canva / Adobe Express photo filters** — broad preset libraries and visual previews, but account/product-oriented and not a developer-style deterministic tool.
2. **Fotor / BeFunky online filters** — many named looks and sliders, but upload-centric workflows and non-local processing.
3. **LunaPic filters** — many classic effects, but server-upload workflow and busy UI.
4. **PineTools image effects** — simple single-effect pages such as grayscale/sepia/invert, but less cohesive preset switching.
5. **Browser/ffmpeg recipes** — reproducible filter chains, but users must know ffmpeg syntax.

## Fit-to-model gaps and decisions

- Built in-model: one-click named presets, deterministic ffmpeg filter chains, broad practical set (sepia/vintage/warm/cool/noir/grayscale/vivid/invert/fade), local browser page, and CLI/chat parity.
- Not built: arbitrary sliders, live thumbnail gallery, AI/Instagram-style proprietary filters, batch upload, and account/cloud workflows. Those are UI/product features rather than a compact deterministic gizza tool.
- Copy/branding: no competitor wording or branded filter names were copied.

## Verification snapshot

- `cargo test --workspace` from `blocks/photo-filter-presets/`
- `wafer build` from `blocks/photo-filter-presets/`
- `wasm-pack build blocks/photo-filter-presets/web --target web --release --out-dir pkg`
- `cargo install --path cli`
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`
- `gizza tool photo-filter-presets ... preset=noir`
- `cd tests && xvfb-run npx playwright test tool-page-photo-filter-presets.spec.ts`
- `npm run test`
