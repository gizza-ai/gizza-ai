# svg-sprite-builder — competitor analysis (2026-06-22)

Tool: combine several standalone `<svg>` documents into one SVG `<symbol>` sprite
sheet referenced with `<use href="#id">`. Pure-Rust, runs on all surfaces
(chat / CLI / page).

## Surfaces verified

- **chat block**: `wafer build` validates + instantiates `target/block.wasm`
  (330 KiB), schema single-sourced from `descriptor()` (drift-guard test passes).
- **CLI**: `gizza tool svg-sprite-builder svgs='…' id_source=id` → correct sprite
  (auto-id fallback + `viewBox` derived from `width`/`height`).
- **page**: Playwright `tool-page-svg-sprite-builder.spec.ts` — 2 specs pass
  (auto ids + `<use>` snippet; `id_source=title` + `prefix`).

## Top competitors surveyed

1. **Fixie Tools — SVG Sprite Generator** (fixie.tools/svg-sprite) — drag/drop,
   single optimized sprite, `<use href="#id">`, 100% client-side, no signup.
2. **CodeShack SVG Sprite Generator** (codeshack.io/svg-sprite-generator) —
   drag/drop multi-file upload, customizable **symbol id prefix** and a
   **default viewBox**.
3. **SVGView SVG Sprite Generator** (svgview.com/svg-sprite-generator) — runs
   entirely in browser, no uploads; output works in React/Vue/plain HTML;
   inline once then `<use href="#icon-id" />`.
4. **Aspose SVG Sprite Generator** (products.aspose.app/svg/svg-sprite-generator)
   — "Inline" vs "Linked" sprite modes.
5. **svgsprit.es** / **Sprite Your SVGs** (sprite-your-svgs.vercel.app) —
   generate + optimize a sprite, reduce HTTP requests, externalize icon data.

## Capability diff (ranked, fit-to-model)

| Capability | Competitors | gizza svg-sprite-builder | Action |
|---|---|---|---|
| `<symbol>` sprite + `<use href="#id">` output | all | yes | core |
| Configurable symbol **id prefix** | CodeShack, others | yes (`prefix`) | shipped |
| Per-symbol **viewBox** preserved | all | yes; **derived from `width`/`height`** when missing (matches CodeShack's "default viewBox" intent, computed per-icon) | shipped |
| Id from source `id` attribute / `<title>` | implicit (filename-based) | yes (`id_source` = auto/id/title) — richer than filename-only | shipped (better fit) |
| Duplicate-id disambiguation | varies | yes (`-2`, `-3`, …) | shipped |
| Hidden/inline wrapper (`aria-hidden`, zero-size) | SVGView/CSS-Tricks pattern | yes (`hidden`, default true) | shipped |
| `<use>` usage snippet in output | docs only | yes (trailing comment) | shipped (extra) |

### Out-of-model (documented, not built)

- **Drag-and-drop multi-FILE upload** (CodeShack/Fixie): the gizza page driver
  takes a single text field, not a multi-file picker. We accept the SVGs as
  concatenated text (paste), which covers chat + CLI + page uniformly. A
  multi-file upload would need a framework change (multi-asset page input).
- **Per-symbol optimization / minification** (svgsprit.es, Sprite Your SVGs):
  gizza already ships a dedicated `svg-optimize` tool; keeping concerns separate
  (optimize each SVG, then sprite) is the better composition than duplicating an
  optimizer here.
- **Filename-derived ids**: filenames aren't available from pasted text; the
  `id`/`title`/auto sources cover the same need without a file model.

## Conclusion

The tool matches every in-model competitor capability (sprite output, id prefix,
viewBox handling, hidden wrapper) and adds richer id sourcing (`id`/`title`) plus
duplicate-id disambiguation and a ready-to-paste `<use>` snippet. The only gaps
are out-of-model (multi-file upload, built-in optimization handled by the sibling
`svg-optimize` tool). No competitor copy, branding, or trademarks were used.

Sources:
- [Fixie Tools — SVG Sprite Generator](https://fixie.tools/svg-sprite)
- [CodeShack SVG Sprite Generator](https://codeshack.io/svg-sprite-generator/)
- [SVGView SVG Sprite Generator](https://svgview.com/svg-sprite-generator)
- [Aspose SVG Sprite Generator](https://products.aspose.app/svg/svg-sprite-generator)
- [svgsprit.es](https://svgsprit.es/)
- [Sprite Your SVGs](https://sprite-your-svgs.vercel.app/)
- [CSS-Tricks — Icon System with SVG Sprites](https://css-tricks.com/svg-sprites-use-better-icon-fonts/)
