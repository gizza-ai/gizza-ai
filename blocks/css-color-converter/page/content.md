## About this tool

CSS Color Converter takes one color value and prints the same renderable color in the formats frontend, design-system, and mobile-app work commonly needs: CSS hex, 8-digit hex with alpha, rgb()/hsl() in either legacy or modern syntax, hwb(), lab(), lch(), oklch(), oklab(), Display P3, HSV/HSB, CMYK, Flutter/Dart and Jetpack Compose `Color(0xAARRGGBB)`, SwiftUI, Android XML `#AARRGGBB`, signed ARGB integer, exact-or-nearest CSS color name, and WCAG contrast on white and black.

Paste `#3498db`, `#f00`, `rgba(52, 152, 219, 0.5)`, `hsl(204 70% 53% / 50%)`, `oklch(65.309% 0.135 242.687)`, `rebeccapurple`, `transparent`, `52, 152, 219`, or `Color(0xFF6750A4)`. The color picker stays linked to the text field for quick swatch picking, while still allowing named colors, transparent values, alpha hex, and app-code snippets that native color inputs cannot hold.

Worked example: `#3498db` with the default legacy syntax returns `#3498db`, `rgb(52, 152, 219)`, `hsl(204.072, 69.874%, 53.137%)`, `oklch(65.309% 0.135 242.687)`, `Color(0xff3498db)`, `#ff3498db`, and contrast ratings of `3.15:1 (AA large text only)` on white and `6.66:1 (AA)` on black.

Use **Write rgb() and hsl() in** to switch between old comma syntax and CSS Color 4 space/slash syntax. Use **Decimal places** to round fractional color-space values from 0 to 8 places. Turn on **Upper-case hex digits** when your codebase prefers `#3498DB` and `0xFF3498DB`.

Limits and edge cases: conversions are deterministic and local to your browser. Channels are quantized to 8-bit sRGB once so every printed notation refers to the same displayed color. OKLCH/OKLab inputs outside the sRGB gamut are clamped to the nearest sRGB color and the output includes a note. CMYK is the usual unmanaged web-tool approximation; print production still needs an ICC profile. WCAG contrast is computed from the opaque sRGB color; translucent colors keep their alpha in code formats, but contrast is not composited against a background.

## FAQ

<details>
<summary>Why are there two 8-digit hex formats?</summary>

CSS writes alpha last as `#RRGGBBAA`, while Flutter, Jetpack Compose, Android color integers, and Android XML often write alpha first as `0xAARRGGBB` or `#AARRGGBB`. This tool prints both forms and labels them so you do not accidentally paste a transparent color where an opaque one was intended.

</details>

<details>
<summary>Does the OKLCH value round-trip back to the same hex color?</summary>

Yes for in-gamut colors at the default precision. The core quantizes to one 8-bit sRGB color first and derives every notation from that value, so converting the emitted OKLCH or OKLab line back lands on the same hex unless you deliberately lower precision enough to throw information away.

</details>

<details>
<summary>Can I paste modern CSS color functions?</summary>

Yes. The parser accepts comma-separated legacy forms such as `rgba(52, 152, 219, 0.5)` and CSS Color 4 forms such as `rgb(52 152 219 / 50%)`, `hsl(204 70% 53% / 50%)`, `hwb(204 20% 14%)`, `oklch(65.309% 0.135 242.687)`, and `oklab(65.309% -0.062 -0.12)`.

</details>

<details>
<summary>Is CMYK color-managed for print?</summary>

No. The CMYK line is the simple device-independent approximation used by lightweight developer tools. It is helpful for estimates and handoffs, but final print work should convert through the printer or paper ICC profile in a color-managed app.

</details>
