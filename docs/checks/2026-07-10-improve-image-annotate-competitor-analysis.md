# image-annotate — competitor analysis (2026-07-10)

Tool: `image-annotate` — "Draw arrows, boxes, highlights, and text labels onto an
image at given coordinates." Pure-Rust (`image` + `fontdue`), chat + CLI surface
(pure-Rust image-bytes output has no standalone page — same shape as
`add-text-to-image` / `image-split-overlay`). Annotations are supplied as a JSON
list so an LLM or CLI user can place many marks in one call at exact pixel
coordinates.

Scan method: two web searches for online image-annotation / screenshot-markup
tools; skimmed the top real tools listed below. All notes are **paraphrased** —
no competitor copy, branding, or trademarks are reproduced.

## Competitors skimmed

1. **imageannotation.org** (Free Image Annotation Tool) — browser-local screenshot markup.
2. **ScreenSnap Pro image-annotation** — outlined/filled shapes, arrows, text, local processing.
3. **WuTools Image Annotator** — freehand, arrows, text, rectangles, circles, highlights, callouts, cover/redact, PNG/JPG export.
4. **ConvertICO Image Annotator** — arrows, text, shapes, highlights, no signup.
5. **Batch Image Tools — Annotate** — arrows, boxes, text, labels, batch across many images.

## Table-stakes matrix (tagged in-model / out-of-model)

| Capability | Competitors | gizza image-annotate | Tag |
|---|---|---|---|
| Arrow (line + arrowhead) | all | `{"type":"arrow","x1","y1","x2","y2"}` with arrowhead at (x2,y2) | **in-model** ✓ |
| Rectangle / box (outlined) | all | `{"type":"box","x","y","w","h"}` hollow rect | **in-model** ✓ |
| Highlight (semi-transparent fill) | all | `{"type":"highlight","x","y","w","h","opacity"}` | **in-model** ✓ |
| Text label / callout text | all | `{"type":"text","x","y","text"}` | **in-model** ✓ |
| Per-annotation color | all | each annotation takes `color` (#rgb/#rrggbb/#rrggbbaa); `color` param is the default | **in-model** ✓ |
| Stroke / line thickness (2–12px) | all | each annotation takes `stroke_width`; `stroke_width` param is the default | **in-model** ✓ |
| Adjustable text size | most | each text annotation takes `font_size`; `font_size` param is the default | **in-model** ✓ |
| Many marks in one pass (batch of shapes) | all | `annotations` is a JSON array — any number of marks in one call | **in-model** ✓ |
| 12 preset color swatches | most | free-form hex; presets are a page-UI affordance and this tool has no page | in-model but N/A (no page) |
| Filled rectangle (solid, not just highlight) | ScreenSnap/WuTools | achievable via `highlight` with `opacity: 1` | **in-model** ✓ (via opacity) |
| Circle / ellipse shape | WuTools/ScreenSnap | not built this pass — hand-rolled ellipse is feasible, deferred to keep the schema focused on the four named primitives | **considered, deferred** |
| Straight line (no arrowhead) | ScreenSnap/WuTools | an `arrow` communicates direction; a plain line is a trivial future variant | **considered, deferred** |
| Dashed / dotted line style | some | solid strokes only this pass; a per-annotation `style` enum is a clean future add | **considered, deferred** |
| Freehand pen drawing | most | needs an interactive canvas (mouse path capture); gizza is coordinate/param-driven | **out-of-model** |
| Blur / pixelate a region (redact) | WuTools/Webvizio | a box-blur region is feasible in pure Rust but is a separate redaction capability, not one of the four named primitives | **considered, deferred** (see `pdf-watermark`/redact family) |
| Numbered pins / step badges | imageannotation.org/WuTools | composable today (a `highlight` circle-ish box + a `text` number); a dedicated `pin` type is a future add | **considered, deferred** |
| JPG output + quality slider | WuTools | output is PNG to preserve alpha under overlays; format switch is a future add | **considered, deferred** |
| Batch across many images | Batch Image Tools | one image per call (single `Input::Image`); loop client-side for batches | **out-of-model** |
| Interactive drag-to-draw editor | all | gizza places marks by exact coordinates via chat/CLI; there is no drawing UI | **out-of-model** |

## Defaults chosen (vs competitor norms)

- Default `color` = `#ff0000` (red) — the universal annotation/attention color across every tool skimmed.
- Default `stroke_width` = `3` px — inside the common 2–12 px range, visible on typical screenshots without dominating.
- Default `font_size` = `24` px — readable label size on a normal-resolution screenshot.
- Default highlight `opacity` = `0.35` — a marker-pen wash that keeps the underlying pixels legible.

## Worked example (documented on the tool + in tests)

`annotations` = `[{"type":"box","x":20,"y":15,"w":120,"h":60},{"type":"arrow","x1":200,"y1":10,"x2":150,"y2":45},{"type":"highlight","x":20,"y":90,"w":160,"h":24,"color":"#ffff00"},{"type":"text","x":22,"y":92,"text":"Look here","color":"#000000"}]`
→ a red box, a red arrow pointing to it, a yellow highlight wash, and a black label — all in one pass.

## UX patterns worth matching (recorded, applied where a coordinate/param tool can)

- **Per-mark override + sensible global default** (competitors keep the last-used color/width as the default): mirrored via the `color`/`stroke_width`/`font_size` params as defaults each annotation can override.
- **One pass, many marks** (competitors let you stack shapes before export): mirrored by the JSON `annotations` array.
- **Attention-red default + hex freedom** (12 swatches + custom): mirrored via a red default and full `#rgb`/`#rrggbb`/`#rrggbbaa` hex acceptance.
- Preset swatches / drag handles / live preview are page-UI affordances; this tool ships chat + CLI only (no page for pure-Rust image-bytes output), so those are N/A here and left to the interactive competitors.
