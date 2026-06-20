# normalize-image — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/normalize-image` — auto-normalize (contrast-stretch) an image
by mapping each channel's used range to full 0–255. Chat + CLI (image input +
image-bytes output → no page, like image-pixelate-censor).

## What competitors do

- **Online "auto contrast / normalize / enhance" sites** (fotor, pinetools auto
  contrast, online image enhancers) — upload, get an enhanced image. Weaknesses:
  the image is **uploaded** (privacy), watermarks/paywalls, and many bundle it
  into a heavy editor.
- **ImageMagick `-normalize` / `-auto-level`** — the reference, local + scriptable,
  but requires installing ImageMagick.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (image crate) → chat SW, CLI,
   and (conceptually) browser. The photo never leaves the device.
2. **Per-channel histogram stretch** to the full dynamic range — the standard
   auto-levels operation that fixes flat/washed-out/low-contrast photos.
3. **Outlier-robust.** `clip_percent` ignores a fraction of the darkest/lightest
   pixels per channel before stretching (ImageMagick `-normalize` style), so a few
   stray hot/black pixels don't defeat the stretch. Default 0 = pure min/max
   (`-auto-level`).
4. **Safe on edge cases.** A flat channel maps to identity (no divide-by-zero), an
   already-full-range image is unchanged, and the alpha channel is preserved.
5. **Lossless PNG output**, chainable via `ref`.

## Honest scope

- Per-channel stretch can shift color balance (it's an auto-color/levels op, by
  design); a luminance-locked mode could be a future option.
- Linear stretch only (no gamma/curve or CLAHE/local contrast).

## Tests

6 core unit tests (on images built in-test, decoded + pixel-probed): a 100–150
range stretches so min→0, max→255, and a midpoint maps linearly (120→102);
already-full-range is unchanged; a flat image is identity (no panic); `clip_percent`
ignores black/white outliers so the bulk [120,140] stretches to [0,255]; alpha is
preserved; bad image errors. Plus the block drift-guard schema test. CLI verified
over the wire on tux.png (valid same-dimension PNG out) — see commit.
