# image-round-avatar — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/image-round-avatar` — crop an image into a circle or
rounded-square avatar with transparent corners (PNG). Chat + CLI (image input +
image-bytes output; the page file-input path is ffmpeg-only — like
add-text-to-image / image-pixelate-censor).

## What competitors do

- **Online avatar / "round image" makers** (crop-circle.com, roundpic,
  imageresizer round) — upload, get a circular crop. Strengths: simple.
  Weaknesses: image is **uploaded to a server** (privacy), watermarks/ads, and
  many output a circle baked onto a white/solid background instead of true
  transparency.
- **Photoshop/Figma** — clip to an ellipse; full control but manual, desktop.
- **CSS `border-radius`** — only affects display, not the actual file (the image
  is still rectangular when downloaded/shared).

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image` crate) compiled to
   wasm: runs in the chat Service Worker and headless via the CLI.
2. **True transparency.** Corners are made fully transparent in the alpha channel
   (PNG), not filled with a background color — so the avatar drops cleanly onto
   any background.
3. **Anti-aliased edges.** A 1px signed-distance coverage mask gives a smooth
   circle/rounded edge instead of a jagged stair-stepped cutout.
4. **Circle and rounded-square in one tool.** `shape=circle` (default) or
   `rounded` with an adjustable corner `radius`. The rounded path uses a proper
   rounded-rectangle SDF (radius = half ⇒ exact circle).
5. **Auto square + optional resize.** The image is center-cropped to the largest
   centered square first, and `size` resizes the output to a target avatar size
   (e.g. 256) in one step.
6. **Chainable + scriptable.** `url`/`ref` input, PNG output that's itself a
   `ref` for downstream tools.

## Honest scope

- Center-crop only (no pan/zoom/offset selection of the crop region yet).
- No ring/border stroke around the avatar yet (could be a future option).

## Tests

6 core unit tests (on images built in-test, decoded back and pixel-probed): shape
parsing; circle clears the corners (alpha 0) and a diagonal near-corner point,
keeps the center opaque, and keeps edge midpoints (the inscribed circle is tangent
to them); rounded keeps the straight edges and clears only the corners; `size`
resizes to a square; a non-square input is center-cropped to a square; bad image
errors. Plus the block drift-guard schema test. CLI verified over the wire on
`tux.png` producing a valid transparent-cornered PNG (see commit).
