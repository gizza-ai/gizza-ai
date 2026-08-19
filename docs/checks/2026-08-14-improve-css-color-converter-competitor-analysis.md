# css-color-converter competitor analysis — 2026-08-14

Backlog item: convert a color between hex, rgb(a), hsl, oklch, and the `0xAARRGGBB` integer form used by Flutter/Dart and Android.

Search query used: `CSS color converter hex rgb hsl oklch Flutter 0xAARRGGBB color tool`.

## Competitors reviewed

| Competitor | Table-stakes observed | UX controls | Model-fit decision |
| --- | --- | --- | --- |
| Num8ers HTML Color Picker | Hex, RGB/RGBA, HSL/HSLA, CMYK, AARRGGBB-style app codes, WCAG contrast, color harmonies/image eyedropper. | Color picker, instant results, contrast/harmony panels. | In-model: color picker text input, hex/RGB/HSL/CMYK/app codes, contrast. Out-of-model for this pure block: image eyedropper and harmony generation; those need browser image interaction or palette generation beyond the backlog ask. |
| OpenReplay RGBA to HEX | Focused alpha conversion between RGBA and 8-digit CSS hex; explains alpha position and accepts transparency. | Simple fields for RGBA channels and alpha, copyable exact output. | In-model: parse RGBA with 0-1 or percentage alpha, emit CSS 8-digit hex with alpha last, exact text output and error handling. |
| Go-Tools Hex to RGB | 3/4/6/8-digit hex, RGB/RGBA, HSL, OKLCH, alpha-pair decoding, round-trip correctness. | Hex input, preset examples, immediate conversion lines. | In-model: short and long hex, alpha, HSL, OKLCH/OKLab, worked examples and preset chips. |

## Requirements carried into this implementation

- Inputs: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, bare hex, CSS named colors, `transparent`, `rgb()`/`rgba()`, modern `rgb(… / …)`, `hsl()`/`hsla()`, modern `hsl(… / …)`, `hwb()`, `oklch()`, `oklab()`, bare `r,g,b` triples, and app snippets such as `Color(0xFF6750A4)`.
- Outputs: CSS hex, CSS hex+alpha, RGB/RGBA, HSL/HSLA, HWB, LAB/LCH, OKLCH/OKLab, Display P3, HSV/HSB, CMYK, Flutter/Dart and Jetpack Compose code, SwiftUI code, Android XML ARGB, signed ARGB integer, exact/nearest CSS name, and WCAG contrast against white/black.
- Controls: hybrid color/text input (`kind = "color"`), syntax enum with readable labels, precision slider, uppercase-hex checkbox, and preset chips for common workflows.
- Defaults: legacy comma CSS syntax, 3 decimal places, lower-case hex.
- Verification matrix: short and long hex, app ARGB input, modern CSS alpha, OKLCH, CSS name, uppercase hex, precision boundary, page deep link, CLI exact-output case.

## Explicitly out of model

- Image eyedropper extraction from uploaded images.
- Palette and color-harmony generation.
- ICC-profile color-managed CMYK conversion.

Those are useful related products, but this block is deterministic pure Rust over a single typed/pasted color value.
