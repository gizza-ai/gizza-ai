## Gamma-correct image midtones

Gamma correction applies a power curve to image midtones. It is different from a
flat brightness slider: pure black and pure white stay fixed, while the tones in
between move. Use it to lift a dark photo without immediately clipping the
highlights, darken washed-out midtones, or correct a subtle channel color cast.

A gamma value above `1` brightens midtones; a value below `1` darkens them. Leave
`gamma_r`, `gamma_g` and `gamma_b` at `1` for a neutral correction, or adjust one
channel to warm/cool the image. `gamma_weight` controls how strongly the gamma
curve applies near highlights: lower it when a strong brighten setting makes
bright areas look too hot.

### Worked example

Upload a dim JPEG and choose the **Brighten dark photo** preset (`gamma = 1.8`,
all channel gammas at `1`, `gamma_weight = 1`). Mid-gray pixels become much
brighter, while black and white remain anchored. If the brightest parts feel too
strong, try **Protect highlights** (`gamma = 2.2`, `gamma_weight = 0.35`) so the
curve fades near the top end. To fix a cool cast, set **Red gamma** to `1.25` and
**Blue gamma** to `0.85`, then export as PNG.

### Limits and edge cases

- Input images up to 8 MiB are processed locally in the browser with ffmpeg.
- `gamma`, `gamma_r`, `gamma_g` and `gamma_b` accept `0.1` through `10`; `1` is
  no change. `gamma_weight` accepts `0` through `1`.
- `keep` preserves the input container. Choosing `png`, `jpg` or `webp` converts
  the output; animated images are treated as stills when converted.
- Gamma correction is not a full raw-photo exposure pipeline. It does not recover
  clipped highlights or shadows that are already pure white/black.
- The operation is RGB/video-filter based, so embedded color profiles and EXIF
  metadata are not preserved.

## FAQ

<details>
<summary>What gamma value should I start with?</summary>

Try `1.6` to `2.0` for a dark photo and `0.5` to `0.8` for washed-out midtones.
`1` is the identity. Small changes are usually best; strong values can make skin
or gradients look unnatural.

</details>

<details>
<summary>How is gamma different from brightness?</summary>

Brightness adds or subtracts from the whole image and can clip black or white
quickly. Gamma bends the middle of the tone curve while keeping the endpoints
fixed, so it is better for midtone-only adjustments.

</details>

<details>
<summary>What does gamma_weight do?</summary>

It fades the gamma correction near highlights. At `1`, the full curve applies.
Lower values reduce the effect on bright pixels, which helps protect light skies,
paper, or reflections when you brighten a dark image.

</details>

<details>
<summary>Can I fix a color cast?</summary>

Yes, use `gamma_r`, `gamma_g` and `gamma_b` for per-channel midtone changes. For
example, raising red gamma and lowering blue gamma warms the image; doing the
opposite cools it.

</details>

<details>
<summary>Is my image uploaded?</summary>

No. The page runs ffmpeg in WebAssembly in your browser tab. The image file stays
on your device.

</details>
