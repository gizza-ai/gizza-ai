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
