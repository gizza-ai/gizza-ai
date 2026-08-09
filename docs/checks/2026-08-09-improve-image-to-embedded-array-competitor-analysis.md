# image-to-embedded-array competitor analysis (2026-08-09)

## Tool goal

Convert an uploaded image into paste-ready C/C++ source for embedded display firmware, especially small TFT/OLED/e-paper projects that need RGB565, RGB888, grayscale, 1-bit mono, or XBM arrays.

## Competitors skimmed

Search query: `image to C array RGB565 XBM embedded display converter`.

| Competitor | Observed table-stakes | UX/control patterns | In-model decision |
| --- | --- | --- | --- |
| IRQKit image-to-RGB565 array converter | PNG/JPG input, RGB565 output, width/height, endian/byte order, C/C++ array output, browser-local conversion. | Upload field, explicit dimension controls, endian selector, copyable code. | Built: `format=rgb565`, resize `width`/`height`, `byte_order=word|big|little`, copyable text JSON response. |
| TeachMeMicro image-to-C-array converter | Upload image, resize to required dimensions, preview/edit pixels, hexadecimal output for embedded displays. Related viewer supports C/C++/Arduino/XBM/RGB565 arrays. | Upload + preview, dimension controls, copy generated hex, pixel editing. | Built: resize, hex/decimal output, C array/Arduino/header/raw styles, RGB565/XBM/mono variants. Out-of-model/not built: interactive pixel editor/preview because this tool is chat+CLI image input without a page. |
| Embedded Unfiltered image-to-C converter | OLED/TFT/e-paper targeting, monochrome and RGB565 style outputs, packing variants for firmware. | Format selector, dimensions, generated C output. | Built: `format=mono|xbm|rgb565|bgr565|rgb888|rgb332|grayscale`, bit order, threshold, invert, dither, C identifier naming. |

## Capability matrix

| Capability | Status | Notes |
| --- | --- | --- |
| PNG/JPEG/GIF/BMP/WebP decode | In model, built | Uses pure Rust `image` crate. CLI verified with PNG and JPEG URLs. |
| Resize to display dimensions | In model, built | `width` and `height`; one dimension preserves aspect ratio; both stretch intentionally. |
| RGB565 output | In model, built | Default format, exact red/blue test covered. |
| BGR565 / byte order variants | In model, built | `bgr565` and `byte_order=word|big|little`. |
| RGB888 / RGB332 / grayscale | In model, built | Useful for libraries and palette-constrained display drivers. |
| 1-bit mono and XBM | In model, built | `mono` uses MSB-first by default; `xbm` uses LSB-first and emits XBM width/height definitions. |
| Threshold, invert, dither | In model, built | Controls common OLED/e-paper conversion tradeoffs. |
| Arduino/PROGMEM/header/raw styles | In model, built | Covers paste-to-sketch, `.h` file, and raw-value workflows. |
| Browser page with live preview/pixel editor | Out of model for this repo/tool shape | Image input and text output are available through chat+CLI; no standalone page was shipped because existing file-input/no-page pattern fits image source tools and avoids a half-verified page. |
| Compression or display-driver-specific metadata | Out of model for v1 | Could be future specialized tools; current output is straightforward arrays users can paste into firmware. |

## Defaults chosen

- `format=rgb565`: most common TFT/LCD embedded display packing.
- `byte_order=word`: matches `uint16_t` APIs such as `pushImage`/`drawRGBBitmap`.
- `style=array`: minimal portable C declaration.
- `number_format=hex`: conventional for firmware arrays.
- `per_line=12`: readable without overly long source lines; `0` means one image row per line.
- `threshold=128`, `dither=false`, `invert=false`: predictable 1-bit baseline with opt-in photo dithering.
- `background=#000000`: common dark-display backdrop for transparent assets.

## Verification notes

- Core tests cover RGB565 exact values, BGR565, split endian bytes, RGB888/RGB332/grayscale, mono/XBM bit ordering, threshold/invert/dither, styles, resizing, parser errors, invalid identifiers, and output caps.
- CLI matrix covered PNG and JPEG URLs plus every advertised pixel format and non-default boolean states.
- No Playwright page spec was added because the tool follows the no-page image-input pattern.
