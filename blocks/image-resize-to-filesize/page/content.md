## About this tool

Some upload forms cap images at an exact file size — "photo must be under 200 KB",
"200 x 200 and below 100 KB", "logo max 50 KB". Guessing a quality slider until you
land under the limit is tedious. This tool does the search for you: give it a target
size in **kilobytes** and it re-encodes your image at several quality levels, measures
each result, and keeps the **highest quality whose file is at or under your budget**.

Everything runs locally in your browser with ffmpeg compiled to WebAssembly — the
image is never uploaded to a server.

### How it works

1. Choose an image (JPG, PNG, WebP, GIF, or BMP).
2. Enter your **Target size (KB)** — for example `200`.
3. Pick the **Output format**: JPEG (widest compatibility) or WebP (usually smaller at
   the same quality).
4. Optionally set a **Max width** to shrink wide images first — helpful when a small
   target can't be met at full resolution.

The tool binary-searches the encoder quality between 5 and 95 (about seven attempts),
then shows the achieved size and the quality it settled on, with a download link.

### Worked example

Upload a 1.8 MB, 3000-px JPEG and set **Target size** to `100` KB with format **JPEG**.
The search converges on roughly quality 47 and produces a file near **98 KB** — under
the 100 KB budget. The status line reads something like: `Done — 98.2 KB at quality 47
(target 100 KB).` If you also set **Max width** to `1200`, the image is shrunk to
1200 px wide first, so the same 100 KB budget buys a higher quality.

### Limits and edge cases

- **Output is JPEG or WebP only.** Both have a quality knob to search. PNG is lossless
  (no quality knob), so it isn't offered as an output — pick JPEG or WebP instead.
- **Target is a ceiling, not an exact value.** The result lands at or just under your
  target; it won't be padded up to match it exactly.
- **Very detailed photos** may not reach an aggressive target at full resolution. When
  even the lowest quality is still too big, the tool returns the smallest version it can
  make and tells you — lower the **Max width** and try again.
- **Max input size** is about 8 MB. Exotic inputs (HEIC, AVIF, TIFF, SVG) may not decode;
  convert them to JPG/PNG/WebP first.
- **1 MB = 1024 KB**, so enter `1024` for a 1 MB budget.

## FAQ

<details>
<summary>How do I compress an image to exactly 100 KB (or 50 KB, 20 KB)?</summary>

Enter the number in the **Target size (KB)** field and choose a format. The tool finds the
best quality that fits at or under that size. It targets a ceiling, so a "100 KB" result is
typically in the high-90s KB — under the limit, which is what upload forms check.

</details>

<details>
<summary>Why is the output only JPEG or WebP — can I keep it as PNG?</summary>

Hitting a target size needs a lossy quality knob to search against. JPEG and WebP have one;
PNG is lossless and has no equivalent, so a "compress PNG to 50 KB" request can only be met
by re-encoding to JPEG or WebP. Choose whichever format your destination accepts — WebP is
usually the smaller of the two at the same visual quality.

</details>

<details>
<summary>What does "Max width" do, and when should I use it?</summary>

It caps the output width in pixels (height scales to match, and it only ever shrinks — a
smaller image is left alone). Reach for it when a tight target can't be met at full
resolution: a 4000-px photo squeezed under 50 KB looks blocky, but the same 50 KB spread
over an 800-px image looks fine. Leave it at `0` to keep the original dimensions.

</details>

<details>
<summary>Is my image uploaded anywhere?</summary>

No. The compression runs entirely in your browser using ffmpeg compiled to WebAssembly. The
image bytes never leave your device, so it works offline and keeps private photos private.

</details>

<details>
<summary>The result is a bit under my target — can I get closer to the exact size?</summary>

The search picks the highest quality that stays at or under your budget, so there's usually a
small gap below the target (the next quality step up would exceed it). That gap is expected and
safe for upload limits. If you need a larger file, raise the target or increase the Max width.

</details>
