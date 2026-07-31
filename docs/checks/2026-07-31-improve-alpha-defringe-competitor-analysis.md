# alpha-defringe — competitor analysis (2026-07-31)

Tool: **alpha-defringe** — removes the dark or light halo/fringe left on the edges of a
cutout against transparency. Type: **pure** (pure-Rust `image` crate, single image in →
transparent PNG out). Surface: **chat + CLI only** — image-input/image-bytes-output pure
tools have no page render mode (same shape as `apply-alpha-mask`, `image-opacity`,
`normalize-image`).

## Scan (WebSearch: "defringe / remove matte fringe / halo / color decontamination")

Paraphrased from public how-to references (Adobe Photoshop help pages, PhotoshopCAFE,
Photoshop Training Channel, tutvid, SitePoint). NO competitor copy/branding reproduced.

The established desktop-editor feature set for this job splits into two named operations,
both of which every reference treats as "defringe":

1. **Defringe by width (color bleed / decontamination).** Replace the color of edge
   ("fringe") pixels with the color of nearby fully-selected (opaque) pixels, up to a
   width given in pixels. Grows the real foreground color outward over the contaminated
   anti-aliased rim so the leftover-background halo disappears. Works for a fringe of ANY
   color — no need to name the old background. This is the general "remove edge halo"
   answer and the primary use case in the description ("dark OR light halo").

2. **Remove Black Matte / Remove White Matte (un-matte).** When the cutout was
   anti-aliased over a KNOWN flat background (black, white, or a green screen), recover the
   true foreground color per edge pixel by algebraically removing that matte color:
   `F = (C − (1−α)·M) / α`. This is the exact inverse of compositing over matte `M`.

## Table-stakes → in-model / out-of-model

| Capability (competitor) | Decision | Where it lands |
|---|---|---|
| Defringe by a pixel WIDTH (bleed inner color outward) | **in-model** | `mode=bleed` + `radius` (1–16) |
| Remove Black / White / custom matte color | **in-model** | `mode=unmatte` + `matte_color` (hex/named, default black) |
| Control what counts as a clean "foreground source" pixel | **in-model** | `threshold` (alpha ≥ = clean source, default 250) |
| Preserve the alpha channel / output alpha-capable format | **in-model** | always PNG (only common 8-bit-alpha raster; matches image-opacity/apply-alpha-mask) |
| Handle any fringe color without naming it | **in-model** | `mode=bleed` (default — needs no color) |
| AI / ML matting or green-screen KEYING (chroma detect) to CREATE the cutout | **out-of-model** | needs a segmentation model; gizza is pure-Rust. This tool CLEANS an existing cutout's edge, it does not create the alpha. (Background removal itself was already skiplisted, see `video-background-remove`.) |
| Interactive brush / manual per-region masking | **out-of-model** | no interactive canvas surface (batch tool) |
| Live before/after preview | **out-of-model** | no page for image-bytes pure tools |

Every table-stake capability lands in the descriptor; the two out-of-model items are
listed, not built.

## Defaults chosen (match competitor conventions)

- `mode = bleed` — the general, color-agnostic default (dark OR light halo, no color to name).
- `radius = 2` — Photoshop's Defringe dialog defaults to a 1–2 px width; 2 clears a typical
  anti-aliased rim without eating into detail. Range 1–16.
- `matte_color = #000000` (black) — "Remove Black Matte" is the most common un-matte case.
  Accepts `#rgb`/`#rrggbb` and named `black`/`white`/`gray`.
- `threshold = 250` — pixels with alpha ≥ 250 are treated as clean foreground sources to
  bleed FROM; anything below is fringe to fix. 0–254.

## Worked examples (verified by unit tests + a real CLI run)

- **Bleed:** a red logo with a green-tinted 1-px anti-aliased rim (alpha ~100) →
  `mode=bleed radius=1` repaints the rim red, alpha untouched → no green halo when placed
  on any background.
- **Un-matte black:** a cutout anti-aliased over black, stored `(128,0,0,128)` at the edge
  → `mode=unmatte matte_color=black` recovers `(255,0,0,128)` — the true red foreground.

## UX controls (competitors)

Desktop tools expose a pixel WIDTH slider and radio/menu choices for the matte color
(black/white/other). Ours maps width→`radius` and the matte choice→`matte_color`; `mode`
selects bleed vs un-matte. No page (image pure tool), so controls are the CLI/chat params.
