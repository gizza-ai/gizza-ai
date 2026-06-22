## About this tool

**Color shades generator** turns a single base color into a full ramp of related
colors. Pick a base color and a mode and it returns every step in HEX, RGB and HSL.

### Modes

- **Scale** — a Tailwind-style **50, 100, 200 … 900, 950** named ramp (the modern
  Tailwind v3.3+/v4 11-weight scale): light tints at the top, dark shades at the
  bottom. The base color is kept exactly at its nearest weight and marked, so the
  ramp reads as a drop-in design-system scale.
- **Tints** — lighten the base step by step toward white.
- **Shades** — darken the base step by step toward black.
- **Tones** — desaturate the base step by step toward a neutral gray.

The **Steps** count controls how many swatches the *tints*, *shades* and *tones*
series produce, from 2 to 12. The **scale** mode ignores it and always returns the
eleven Tailwind weights (50-950).

### Input formats

Enter the base color as a `#hex` (3 or 6 digits), an `rgb()` value, or an `hsl()`
value — they are all accepted and normalized.

### Privacy

Everything runs **in your browser** via WebAssembly. You can also run it from the
[gizza CLI](/) or inside a gizza chat (which return the ramp as structured JSON).

### Common uses

- Build a Tailwind / design-system color scale (50-900) from one brand color.
- Generate a set of hover/active/disabled shades for a UI component.
- Find a softer "tone" of a color that's too saturated for backgrounds.
