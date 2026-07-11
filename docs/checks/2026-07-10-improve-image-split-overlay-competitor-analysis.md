# image-split-overlay — competitor analysis (2026-07-10)

Tool function: take **two** images (A and B) and output a **single** static image
where a chosen split line (vertical / horizontal / diagonal) reveals image A on one
side and image B on the other — the classic "before/after reveal" frame, but flattened
to one shareable image rather than an interactive drag slider.

## Competitors skimmed (paraphrased, no copy/branding reproduced)

1. **Scanly — Image Comparison Slider (before/after).** Overlays two same-scene images
   and drags a divider to reveal one over the other; toggles horizontal (left/right) vs
   vertical (top/bottom) orientation; suggests horizontal for portrait, vertical for
   landscape. Output is an *interactive* JS slider widget.
2. **Pi7 Collage — 2 photos in one frame.** Four layouts: side-by-side, top-bottom,
   big+small, and **diagonal split**. Free, instant, high-res export, no watermark.
3. **ROCKIMG / Picture Split (iOS) — before/after merge.** Merge two photos into a
   before/after frame; vertical split; local/private processing.
4. **Image Combiner / PixelPanda (context).** Side-by-side / stacked / grid merges with
   adjustable spacing; browser-local, no upload.

## Table-stakes → decision (every item lands in the descriptor OR is listed here)

| Capability | Decision | Where |
|---|---|---|
| Vertical split (A left / B right) | in-model | `orientation=vertical` |
| Horizontal split (A top / B bottom) | in-model | `orientation=horizontal` |
| Diagonal split (both diagonals) | in-model | `orientation=diagonal`/`diagonal-reverse` |
| Adjustable split position | in-model | `position` (0–100 %) |
| Divider line between sides (width + color) | in-model | `divider_width`, `divider_color` |
| Handle two differently-sized images | in-model | `fit=stretch\|cover` (canvas = A's size) |
| Output format choice | in-model | `format=png\|jpeg` |
| Local / private processing (no upload) | in-model (native) | gizza runs client-side/CLI |
| High-res export | in-model | full-res up to a 40 MP guard |

## Out-of-model (listed, not built)

- **Interactive drag slider widget** — competitors' headline feature is a live draggable
  handle; gizza outputs a single *static* flattened image by design (the task spec). Not a
  regression: the static split image is the deliverable. A live slider is a JS-widget
  concern, out of a pure-compute block's scope.
- **Per-side text labels ("Before"/"After").** Feasible in-model (font rendering exists in
  `text-banner-image`/`add-text-to-image`), but deferred from v1 to keep the block focused;
  recorded here so it is not silently dropped. Candidate for a future improve pass.
- **Feathered/soft split edge, drop-shadow on the divider.** Cosmetic; deferred.

## UX controls competitors ship (mapped to gizza surfaces)

- Orientation toggle → `orientation` enum (chat/CLI); would be a `<select>` on a page.
- Position slider → `position` number 0–100. (No page: two-arbitrary-image input is not
  expressible in the single-file-upload page generator, so this ships **chat + CLI**, like
  `image-collage`. Recorded so the missing page is explicit, not an oversight.)
- Divider color → hex `divider_color` (accepts `#rgb`/`#rrggbb`/`#rrggbbaa`).

## Surface note

Two arbitrary full images are required. The page generator's `[[input]] source="file"`
is a single upload and there is no clean dual-image control, so — matching the established
multi-image pattern (`image-collage`, `gif-from-images`) — this ships as a **chat + CLI**
tool with **no standalone page**. Both images are passed as an ordered `images` list
(item 1 = side A, item 2 = side B), each a `url` or a prior-tool `ref`.
