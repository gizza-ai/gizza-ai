# image-pixelate-censor — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/image-pixelate-censor` — censor a rectangular region of an
image by pixelating (mosaic) or blurring it. Chat + CLI (image input + image-bytes
output; the page file-input path is ffmpeg-only — like add-text-to-image).

## What competitors do

- **Online censor/blur tools** (redacted.app, pixelate.org, facepixelizer,
  online blur-image sites) — upload, draw a box, download. Strengths: interactive
  box-drawing. Weaknesses: the image (often containing **sensitive** content — the
  whole point of censoring) is **uploaded to a server**, which is the worst place
  to send something you're trying to redact. Ads/watermarks common.
- **Photoshop/GIMP** — full control but desktop-bound, manual, not automatable.
- **ImageMagick** (`-region ... -scale 10% -scale 1000%`) — local + scriptable
  but arcane.

## How this tool competes / improves

1. **Runs locally — the sensitive image is never uploaded.** Pure-Rust (`image`
   crate) compiled to wasm: runs in the chat Service Worker and headless via the
   CLI. For a redaction tool this privacy property is the headline feature.
2. **Two proven redaction styles.** Pixelate (mosaic — irreversible blocky
   averaging) or blur (gaussian). Pixelate with a large tile fully flattens the
   region (verified: the whole region becomes one flat color).
3. **Region-scoped, non-destructive elsewhere.** Only the given rectangle is
   altered; every pixel outside it is byte-identical to the source (verified).
4. **Forgiving bounds.** The region is clamped to the image, so an over-large box
   just censors to the edge instead of erroring.
5. **Tunable strength.** Mosaic tile size / blur radius is adjustable, with sane
   defaults (16 px tiles / sigma 12).
6. **Chainable + scriptable.** Coordinates as data (not a manual drag), `url`/`ref`
   input, PNG output that's itself a `ref` — usable by an agent or pipeline.

## Honest scope

- Single rectangular region per call (run again to censor multiple areas, or this
  could later accept a list of boxes).
- No automatic face/plate **detection** — you supply the box (detection would need
  an ML model, out of the gizza pure+ffmpeg model). Pairs well with
  `image-info`/`image-color-picker` for locating coordinates.
- PNG output (lossless, avoids re-compressing the un-censored areas).

## Tests

5 core unit tests (on images built in-test): mode parsing; pixelate a whole region
with a large tile → every region pixel becomes one flat averaged color and differs
from the original sharp pattern; blur a region → a pixel **outside** the region is
byte-identical to the source (region-scoped); an over-large region is clamped to
the image (no panic, dims preserved); error cases (origin outside image, zero
width, undecodable bytes). Plus the block drift-guard schema test. CLI verified
over the wire on `tux.png` (see commit) producing a valid same-dimension PNG.
