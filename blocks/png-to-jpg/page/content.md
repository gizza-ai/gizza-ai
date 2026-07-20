## About this tool

PNG keeps transparency; JPG does not. Most converters silently flatten a transparent PNG onto
white — fine for a white page, but it leaves an ugly halo the moment the JPG sits on anything
darker. This converter makes that step explicit: pick the **background color** yourself (the
swatch, a hex value like `#ffffff` or `#0f0`, or a CSS name like `navy`), and every transparent
pixel is flattened onto it before the JPEG encode. Semi-transparent pixels — soft shadows,
anti-aliased edges — blend onto the color exactly the way a browser would render them.

Conversion runs entirely in your browser with WebAssembly ffmpeg: the image is never uploaded
to a server.

### Worked example

Take a 64×64 logo PNG whose left third is fully transparent, middle third is 50%-opaque red,
and right third is solid red:

- Convert with the defaults (background `#ffffff`, quality `85`) → a 64×64 JPG whose left
  third is pure white `#ffffff`, middle third is the blend `#ff8080` (50% red over white),
  and right third stays `#ff0000`.
- Convert again with background `#000000` → left third black, middle third `#800000`
  (50% red over black), right third still `#ff0000`.

Same image, two different flattens — that is the choice JPG forces, and the one this tool
puts in your hands.

### Limits and edge cases

- **Quality** is `1–100` (default `85`); values outside that range are rejected with an error.
  `100` is near-lossless but biggest, `60–80` is the usual size/quality trade.
- **Any input format** ffmpeg decodes works (PNG, WebP, GIF, BMP, TIFF, even JPG itself);
  **animated** inputs (GIF/APNG/animated WebP) convert their **first frame** — JPG is a
  single still image.
- Colors accept CSS names (`white`, `black`, `navy`, …) and hex as `#RGB`, `#RRGGBB`, or
  `0xRRGGBB`. Unknown colors produce an error, never a silent fallback.
- On this page the file is processed locally, so size is bounded only by browser memory.
  The command-line/chat surface fetches by URL and caps input at 8 MiB.
- JPEG always re-encodes lossily — converting the same file to JPG repeatedly compounds the
  loss. Keep the PNG as your master copy.

## FAQ

<details>
<summary>Why does my transparent PNG end up with a white background?</summary>

JPEG has no alpha channel, so *something* has to fill the transparent areas. White is the
default here (and in most converters) because it matches how browsers render a transparent
PNG on a typical page. If the JPG will sit on a dark site, a slide, or a colored banner, set
the background color to match — that avoids the classic white-halo effect around logos and
icons.

</details>

<details>
<summary>Can I keep the transparency instead of flattening it?</summary>

Not in a JPG — the format simply cannot store an alpha channel. If you need transparency,
stay with PNG or convert to WebP, both of which support it. This tool is for the opposite
case: you need a JPG (smaller file, wider compatibility with older systems) and want control
over what the transparent areas become.

</details>

<details>
<summary>What happens to semi-transparent pixels like shadows and smooth edges?</summary>

They are alpha-blended onto your background color, not snapped to fully-visible or
fully-erased. A 50%-opaque red pixel over a white background becomes `#ff8080`, over black it
becomes `#800000` — the same math a browser uses when compositing the PNG, so the JPG looks
identical to what you saw on screen.

</details>

<details>
<summary>What JPEG quality should I pick?</summary>

The default `85` is visually clean for photos and screenshots at a reasonable size. Drop to
`60–80` when file size matters more than pixel-level fidelity, and use `95–100` for archival
or print-bound images. Flat graphics with hard edges (logos, text) show JPEG artifacts
soonest — if quality 85 looks smudged, raise it or reconsider whether PNG is the better
target.

</details>

<details>
<summary>Is my image uploaded to a server?</summary>

No. The page runs ffmpeg compiled to WebAssembly inside your browser, so the conversion
happens on your machine and the image never leaves it. The command-line surface instead
fetches the image from the URL you pass (capped at 8 MiB) and converts locally too.

</details>
