# image-oil-painting — competitor analysis (2026-09-04)

Scan run before implementation, per `/create-next-tool` step 4. Findings are paraphrased only; competitor names, copy, and branding are not reused in page or tool copy.

Search: "online oil painting photo effect filter brush radius intensity levels canvas texture tool".

## Competitor profiles

### 1. vayce.app — oil painting effect
- Parameters/options: brush size, paint/vibrance strength, canvas texture.
- Input/output: browser image upload; downloadable image output.
- UX patterns: sliders for effect strength controls, local preview/download flow, shareable/deep-link style tool pages.
- Table-stakes notes: edge-preserving painterly brush filter, colour boost, optional canvas texture.

### 2. imageonline.io — oil painting filter
- Parameters/options: adjustable brush radius and effect intensity.
- Input/output: upload a photo and download raster output.
- UX patterns: upload/dropzone, explicit apply/generate button, slider-style effect controls, download action.
- Table-stakes notes: radius/intensity are the main user-facing controls; examples focus on portraits and landscapes.

### 3. getzenquery.com — photo to oil painting
- Parameters/options: brush size, stroke intensity, canvas texture, colour palette/style controls.
- Input/output: upload image, preview transformed art, download output.
- UX patterns: user-friendly presets/adjustable controls around brushwork and texture.
- Table-stakes notes: exposes both brush geometry and colour/style tuning; positions texture as optional finishing.

## Table-stakes → decisions

| # | Table-stake | Fit | Where it landed |
| --- | --- | --- | --- |
| 1 | Brush size / radius slider | in-model | `radius` integer 1–12, default 4 |
| 2 | Effect / stroke intensity | in-model | `brush_strength` number 0–1, default 0.85 |
| 3 | Brightness/intensity bucket count | in-model | `intensity_levels` integer 8–64, default 24 |
| 4 | Colour vibrance / palette boost | in-model | `saturation` number 0.5–2.0, default 1.1 |
| 5 | Canvas/linen texture | in-model | `canvas_texture` number 0–1, default 0 |
| 6 | Regenerate/different brushwork | in-model | `seed` integer, deterministic repaint variations |
| 7 | Browser/local image transform | in-model | pure-Rust `image` implementation, no ML model |
| 8 | Raster image output | in-model | PNG media envelope at original dimensions |
| 9 | Download/preview page controls | out-of-model for this block surface | This repo's comparable pure image media blocks (`image-to-pixel-art`, `image-low-poly`) are chat/CLI media tools without generated standalone pages. |
| 10 | Bulk upload/history/accounts | out-of-model | Product/account workflow, not a single deterministic block. |

## Out-of-model / intentionally not built

- Multiple painterly style families (watercolour, sketch, cartoon) in one tool. This backlog item is specifically non-ML oil painting; separate style tools should remain separate.
- Server-side batch history or account-gated galleries. Gizza blocks are deterministic local/chat tools.
- Live slider recompute on every drag. The block returns a deterministic PNG for each requested parameter set; UI behaviour belongs to the consuming surface.
