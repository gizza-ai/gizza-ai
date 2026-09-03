# image-low-poly — competitor analysis (2026-09-04)

Scan run **before** implementing, per `/create-next-tool` step 4. All competitor findings are
**paraphrased** — no competitor copy, branding, or trademarks are reproduced or reused.

Search: "low poly image generator online convert photo to triangles" (WebSearch).

Eight candidate tools surfaced. Two of the ranked picks were unreachable to the fetcher and were
**replaced** rather than run short (per the recipe): `pixlane.media/low-poly-generator/` → HTTP 403,
`pikdraw.com/free-online-low-poly-art-generator` → HTTP 403. The three profiled below are the top
three reachable, real tool pages (not listicles, not login walls).

---

## Competitor profiles

### 1. imageonline.io — Low Poly Generator
- **URL:** https://imageonline.io/low-poly-generator/
- **Params / options:**
  | name | type | range | default |
  | --- | --- | --- | --- |
  | Triangle Count | slider | 100–5000 | 1000 |
  | Edge Detection | slider | (unstated) | 30 |
  | Color Accuracy | slider | (unstated) | 1 |
- **Input formats:** image upload (specific formats not stated)
- **Output formats:** PNG, JPG, WebP
- **UX patterns:** drag-and-drop upload, clipboard paste, explicit *Generate* button, loading state,
  download-format menu, copy-to-clipboard.
- **Output quality:** edge detection followed by Delaunay triangulation; per-region colour fill.
- **Limits:** none stated.
- **Copy/SEO angles (paraphrased):** history/definition of the low-poly aesthetic; how the algorithm
  works; density guidance with concrete numbers (it suggests roughly 1000–2000 triangles for
  portraits); use cases such as social posts, video thumbnails, site backgrounds, poster art and
  avatars; which subjects suit the effect (portraits, animals, landscapes).
- **Free vs paid:** free.

### 2. siutil.com — Low Poly Image Generator
- **URL:** https://siutil.com/low-poly/
- **Params / options:**
  | name | type | range | default |
  | --- | --- | --- | --- |
  | Detail (triangles) | slider | (unstated) | 800 |
  | Edge Focus | slider | (unstated) | 60 |
- **Input formats:** PNG, JPG, plus clipboard paste.
- **Output formats:** PNG (opaque — no transparency).
- **UX patterns:** *Regenerate* button that reshuffles point placement to produce variations;
  *Reset settings*; fully local, browser-only processing.
- **Output quality:** explains that higher detail → smaller triangles and a result closer to the
  original, lower detail → bolder and more abstract; higher edge focus concentrates points along
  contours and high-contrast areas while flat regions collapse into large triangles.
- **Limits:** none stated.
- **Free vs paid:** free.

### 3. vayce.app — Low-Poly Art Effect
- **URL:** https://vayce.app/tools/low-poly-art-effect/
- **Params / options:**
  | name | type | range | default |
  | --- | --- | --- | --- |
  | Polygon Scale | slider | % of image size | (unstated) |
  | Geometry Variance | slider | 0–100% | (unstated) |
  | Wireframe Opacity | slider | 0–100% | (unstated) |
- **Input formats:** image upload, drag-and-drop, clipboard paste.
- **Output formats:** downloadable image; shareable link with settings encoded in the URL.
- **UX patterns:** settings persisted to URL query params (shareable/deep-linkable), a
  randomize ("surprise me") button, chaining into other effects.
- **Output quality:** describes mapping the image onto a 2D triangle mesh for a retro-3D /
  minimalist graphic look.
- **Limits:** none stated.
- **FAQ topics:** how the effect works mechanically; why faint seams appear between triangles;
  how to get fully random triangles.
- **Free vs paid:** free.

---

## Table-stakes → decision

Every table-stake below lands in our descriptor or in the out-of-model list. Nothing is dropped
silently.

| # | Table-stake (who) | Fit | Where it landed |
| --- | --- | --- | --- |
| 1 | Triangle-count / detail slider (all 3) | **in-model** | `triangles` — integer 50–4000, default 800, page `kind = "slider"` |
| 2 | Edge detection / edge focus (imageonline, siutil) | **in-model** | `edge_focus` — integer 0–100, default 60, page slider |
| 3 | Colour-accuracy control (imageonline) | **in-model** | `color_mode` — enum `average` \| `centroid` |
| 4 | Wireframe / stroke overlay (vayce; pixlane per search snippet) | **in-model** | `stroke` (colour, page `kind = "color"`) + `stroke_width` |
| 5 | Regenerate / randomize / geometry variance (siutil, vayce) | **in-model** | `seed` — integer, deterministic reshuffle; changing it is our "regenerate" |
| 6 | Raster PNG output (all 3) | **in-model** | `output` enum `svg` \| `png` |
| 7 | Vector/SVG output (pixlane snippet mentions strokes + scalable art) | **in-model** | `output = svg` is our **default** — competitors ship raster only |
| 8 | Deep-linkable settings in the URL (vayce) | **in-model, already platform** | the generator pre-fills every param from the query string; covered by the deep-link Playwright case |
| 9 | Preset chips / density guidance (imageonline copy) | **in-model** | five `[[example]]` chips (portrait, abstract, wireframe poster, detailed, PNG) |
| 10 | Drag-and-drop + clipboard paste upload (all 3) | **in-model, already platform** | the file dropzone + `paste` handler in `page/custom.js` |
| 11 | Reset settings (siutil) | **in-model, already platform** | the generator renders Reset on every page |
| 12 | Local/no-upload processing (siutil, vayce) | **in-model, already platform** | wasm runs in the browser; stated in the page copy |
| 13 | Explicit stated limits | **in-model — gap in ALL 3** | none of the three state any limit; we state input size, dimension and raster caps on the page |

### Out-of-model — considered, not built
- **JPG / WebP export menus** (imageonline). Our raster path emits PNG; adding lossy encoders for an
  art filter whose value is flat colour regions costs binary size for no quality gain, and the
  browser download plus any converter covers it. Listed, not built.
- **Live drag-preview while a slider moves** (vayce). Each run is a full decode + triangulate; a
  per-frame recompute is a UX regression on large photos. The platform's one-run-per-slider-release
  behaviour is kept.
- **Effect chaining into other filters** (vayce). That is a multi-tool pipeline product feature, not
  a single browser-local block.
- **Server-side batch / account-gated history.** Out of the browser-local, no-account model.

### Considered, rejected (in-model but declined)
- **A separate "polygon scale %" control** (vayce) alongside `triangles`. It is the same knob in
  different units; shipping both would be redundant schema surface. `triangles` is the unit all
  other competitors and the copy guidance use.
