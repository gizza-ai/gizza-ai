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

## FAQ

<details>
<summary>Which color formats can I paste as the base color?</summary>

Hex (`#1e90ff`, short `#fa0`, and even 4- or 8-digit hex — the alpha digits
are accepted but the alpha channel is ignored), `rgb(30, 144, 255)` including
percentage components, and `hsl(210, 100%, 56%)`. Whatever you enter is
normalized, and every output step is reported in HEX, RGB and HSL at once.

</details>

<details>
<summary>Why doesn't the Steps setting change the scale output?</summary>

The **scale** mode always returns the eleven Tailwind-style weights
(50, 100, … 900, 950) so it drops straight into a design system — it ignores
Steps by design. Steps only applies to **tints**, **shades** and **tones**,
and is clamped to the 2–12 range.

</details>

<details>
<summary>Will my exact base color appear in the generated ramp?</summary>

In scale mode, yes: the tool finds the weight whose lightness is closest to
your base color, pins your exact color there, and marks that step as the base.
In tints/shades/tones the base is the starting point of the series, with each
step moving toward white, black, or gray respectively.

</details>
