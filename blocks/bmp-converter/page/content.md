## Convert images to (and from) uncompressed BMP

Pick an image and a bit depth — an uncompressed Windows BMP is written with
ffmpeg, entirely in your browser. BMP stores raw pixels with no compression
and no metadata, which is exactly why it is still asked for: embedded displays,
microcontroller and e-paper toolchains, older Windows software, print RIPs and
label printers often accept nothing else. Because there is no compression, the
**bit depth** is what decides both file size and how the image looks.

Set **Output format** to `PNG` or `JPEG` instead to go the other way and turn a
BMP into a modern, compressed format.

### Worked example

Take a 640×480 photo and leave everything at the defaults (**BMP**, **24-bit**).
The result is `out.bmp`, exactly 640×480, at `640 × 480 × 3 bytes + 54 bytes of
header = 921,654 bytes` — every BMP at a given depth and size is the same size,
because nothing is compressed.

Switch **BMP bit depth** to `8-bit` and the same photo becomes
`640 × 480 × 1 byte + 54 + 1,024 bytes of palette = 308,278 bytes` — a third of
the size. The 256 colours are chosen adaptively from your image, and
Floyd–Steinberg dithering (the default) blends them so skies and skin tones
don't band. Drop **Palette colours** to `16` and set **Dithering** to `none` to
get a hard-edged, poster-like 16-colour bitmap instead.

At `1-bit` the same photo is `640 × 480 ÷ 8 + 62 = 38,462 bytes` of pure black
and white — the format a thermal label printer or a monochrome e-paper panel
wants.

### Which bit depth should I pick?

| Depth | Bytes per pixel | Good for |
| --- | --- | --- |
| **32-bit** | 4 | The only depth that keeps transparency (BGRA). |
| **24-bit** | 3 | The safe default — true colour, read by essentially everything. |
| **16-bit 5:6:5** | 2 | Half the size of 24-bit; the layout most embedded displays expect. Written with BITFIELDS colour masks. |
| **16-bit 5:5:5** | 2 | The older high-colour layout; the most widely readable 16-bit BMP. |
| **8-bit** | 1 | 256 adaptive colours, or true greyscale with **Convert to greyscale** ticked. |
| **1-bit** | ⅛ | Black and white for label printers, e-paper and fax-style output. |

### Limits and edge cases

- Input files up to **8 MiB**; any image format ffmpeg can decode works (PNG,
  JPEG, WebP, GIF, BMP, TIFF, …). Animated inputs contribute their first frame.
- The output always has the **exact pixel dimensions of the input** — this tool
  never resizes, crops or changes DPI.
- Output is **uncompressed** (`BI_RGB`). The one exception is 16-bit 5:6:5,
  which uses `BI_BITFIELDS` colour masks — still raw, uncompressed pixel data,
  just with an explicit mask table. RLE4/RLE8 compression is not written.
- Rows are stored **bottom-up**, the conventional BMP layout.
- **Only 32-bit BMP and PNG output keep transparency.** At every other depth,
  and for JPEG, transparent pixels are flattened onto **Background behind
  transparency** first (`#ffffff` by default). Both `#f00` and `#ff0000` work,
  as do colour names like `white` or `navy`.
- **Palette colours** and **Dithering** only affect 8-bit indexed output;
  **Dithering** additionally controls the 1-bit threshold. They are ignored at
  16, 24 and 32-bit, where no colours are discarded.
- **4-bit (16-colour) BMP is not offered.** The encoder here writes 1, 8, 16, 24
  and 32-bit only; asking for 16 colours at 8-bit depth gives the same palette
  with one byte per pixel instead of a half-byte.
- Bad values are rejected with the expected range named — e.g. a palette size of
  `300` reports that colours must be between 2 and 256.

## FAQ

<details>
<summary>Why is my BMP so much bigger than the PNG or JPEG I started with?</summary>

Because BMP does not compress. A PNG of a screenshot might be 60 KB; the same
image as a 24-bit BMP is width × height × 3 bytes no matter what it contains —
a blank white page and a detailed photo of the same size produce byte-identical
file sizes. That predictability is the point for hardware that reads pixels
straight out of the file. If you want a smaller file, drop the bit depth
(8-bit is a third the size of 24-bit, 1-bit is a twenty-fourth) or switch
**Output format** to PNG.

</details>

<details>
<summary>When should I turn dithering off?</summary>

Turn it off for flat-colour artwork — logos, icons, screenshots, pixel art,
line drawings. Those images already contain few colours, so reducing the
palette loses nothing and dithering would only sprinkle noise across areas that
should stay perfectly flat. Keep dithering on (Floyd–Steinberg by default) for
photographs and anything with gradients, where 256 colours would otherwise show
visible banding across skies and skin tones.

</details>

<details>
<summary>What is the difference between the two 16-bit options?</summary>

Both store two bytes per pixel, but they split those 16 bits differently.
**5:5:5** gives red, green and blue five bits each (32 levels each) and leaves
one bit unused; it is the original high-colour layout and the one older
software is most likely to read. **5:6:5** gives green an extra bit — the eye
is most sensitive to green — for smoother gradients, and is what most embedded
LCD and TFT panels expect. The 5:6:5 file records its channel layout in a
BITFIELDS mask table, so a reader that only understands plain 5:5:5 may reject
it. If in doubt, pick 5:5:5.

</details>

<details>
<summary>What happens to transparent areas of a PNG?</summary>

Only 32-bit BMP stores an alpha channel. At any other depth the transparent
pixels are composited onto the **Background behind transparency** colour first,
which defaults to white — so a transparent logo comes out on a white square. Set
that field to match whatever the image will sit on, or choose 32-bit to keep the
transparency intact. PNG output keeps transparency untouched; JPEG output always
flattens, because JPEG has no alpha channel.

</details>

<details>
<summary>Can I convert a BMP back into PNG or JPEG here?</summary>

Yes — that is what **Output format** is for. Upload the BMP, set the format to
`PNG` (lossless, keeps transparency) or `JPEG` (much smaller, no transparency,
written at a high quality setting). The bit depth, palette and dithering fields
are ignored in that direction, because PNG and JPEG choose their own encoding.

</details>

<details>
<summary>Is my image uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then does the whole
conversion locally in the browser tab — the image never leaves your device, and
the converted file is generated in memory for you to download.

</details>
