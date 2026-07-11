# auto-white-balance — competitor analysis (2026-07-10)

Function: remove color casts from a photo by neutralizing its average color
(gray-world) or its brightest pixels (white-patch), restoring natural colors.

## Competitors skimmed (paraphrased; no copy/branding reproduced)

- **Imagen — White Balance Fixer**: one-click in-browser cast correction that can
  also warm/cool a photo; free, no watermark.
- **Musely — AI Auto White Balance Corrector**: AI detects the dominant light
  source and removes yellow/orange (incandescent), green (fluorescent), and blue
  (shade/overcast) casts automatically.
- **LightX — White Balance tool**: manual Temperature and Tint sliders to correct
  imbalance by hand.
- **Aragon / Media.io — Auto Color Correction**: single-click "auto color" that
  balances white balance along with exposure/contrast/saturation.
- **Capture One / Lightroom — White Balance tool**: classic desktop model — an
  eyedropper that sets balance from a clicked neutral-gray area, plus Temp/Tint.

## Table-stakes → decision (in-model = shipped in the descriptor)

| Capability | Competitor pattern | Decision |
|---|---|---|
| One-click auto correction | Imagen/Media.io/Aragon "auto" | **in-model** — `method=gray-world` (default): scales each channel so the average becomes neutral gray. |
| Neutralize on the highlights | "white point" / brightest-area balance | **in-model** — `method=white-patch`: scales each channel so its brightest pixel becomes white (White-Patch Retinex). |
| Correction strength / intensity | sliders that let you dial back the effect | **in-model** — `strength` 0–100 (default 100) blends the corrected result with the original. |
| Alpha / transparency preserved | (expected of any image tool) | **in-model** — RGBA passthrough, alpha untouched. |
| Gray-point eyedropper (click a neutral area) | Lightroom/Capture One eyedropper | **out-of-scope (UI)** — needs an interactive canvas click; the auto `white-patch`/`gray-world` methods approximate it headlessly. |
| Manual Temperature / Tint sliders | LightX/Capture One | **out-of-scope** — that is manual color-temperature grading, a separate tool, not auto cast removal. |
| AI light-source / scene detection | Musely/Aragon | **out-of-model** — gizza has no ML model loader; the statistical gray-world/white-patch methods are the in-model equivalent. |
| Bundled exposure/contrast/saturation "auto" | Aragon/Media.io | **out-of-scope** — those are separate gizza tools (normalize-image, image-brightness-contrast); this tool does white balance only. |

## Worked example baked into the tool

A photo shot under warm indoor light has an average color biased toward
red/orange. Gray-world computes the mean R/G/B, then scales each channel so the
overall average lands on neutral gray, removing the orange cast. `strength=60`
applies a gentler correction that keeps some of the original mood.

## Surfaces

Pure-Rust (`image` crate), image-bytes output → chat + CLI only, **no page**
(same shape as normalize-image / image-false-color — image-bytes tools have no
page render mode). Playwright is therefore not applicable.
