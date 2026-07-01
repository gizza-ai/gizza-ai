## About this tool

**Color palette generator** turns a single base color into a coordinated palette
using classic color-theory harmonies. Pick a base color and a scheme and it
returns every swatch in HEX, RGB and HSL.

### Schemes

- **Complementary** — the base plus its opposite on the color wheel (high contrast).
- **Analogous** — neighbours of the base, spaced 30° apart (calm, cohesive).
- **Triadic** — three colors evenly spaced 120° apart (vivid and balanced).
- **Split-complementary** — the base plus the two neighbours of its complement.
- **Tetradic** — a four-color rectangle of two complementary pairs.
- **Square** — four colors evenly spaced 90° apart.
- **Monochromatic** — one hue at several lightness levels.
- **Shades** — the base darkened toward black.
- **Tints** — the base lightened toward white.

The **Colors** count controls how many swatches the *series* schemes (analogous,
monochromatic, shades, tints) produce, from 2 to 12. The fixed harmony schemes
(complementary, triadic, square, etc.) always return their natural number of
colors.

### Input formats

Enter the base color as a `#hex` (3 or 6 digits), an `rgb()` value, or an `hsl()`
value — they are all accepted and normalized.

### Privacy

Everything runs **in your browser** via WebAssembly. You can also run it from the
[gizza CLI](/) or inside a gizza chat (which return the palette as structured JSON).

### Common uses

- Build a brand or UI palette around one accent color.
- Find a complementary or triadic accent for a design.
- Generate a monochromatic ramp of tints/shades for a component library.

## FAQ

<details>
<summary>Which color formats does the base color accept?</summary>

Any of `#hex` with 3, 4, 6, or 8 digits, `rgb(...)` (with numbers or percentages),
or `hsl(...)`. A 4- or 8-digit hex is accepted too — its alpha channel is simply
ignored, and the palette is computed from the opaque RGB value. Whatever you enter
is normalized, and every swatch comes back in HEX, RGB, and HSL.

</details>

<details>
<summary>Why doesn't the Colors count change my palette?</summary>

The count (2–12, default 5) only applies to the *series* schemes: analogous,
monochromatic, shades, and tints. The fixed harmonies always return their natural
size — complementary gives 2 colors, triadic and split-complementary give 3,
tetradic and square give 4 — no matter what the count is set to.

</details>

<details>
<summary>Which scheme should I pick for a UI or brand palette?</summary>

Start with **monochromatic** or **tints/shades** for a ramp around one brand color
(great for component libraries), **analogous** for a calm cohesive set, and
**complementary** or **split-complementary** when you need a contrasting accent.
Triadic and square give vivid, evenly balanced sets for illustration work.

</details>

<details>
<summary>Is my color data sent anywhere?</summary>

No. The palette math runs entirely in your browser via WebAssembly — no server
call, no account. The same tool is also callable from the gizza CLI or chat, where
it returns the palette as structured JSON.

</details>
