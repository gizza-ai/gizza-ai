## What this tool does

Generate CSS gradients from your own colors, right in your browser. Pick a
**Gradient type**, set an **Angle** or **Shape**, list your **Color stops**, and
copy the ready-to-paste `background-image` declaration. Nothing is sent to a
server — it runs locally, works offline, and needs no sign-up.

## Gradient types

| Type | What it does | Example output |
| --- | --- | --- |
| **linear** (default) | A straight color band along an angle | `linear-gradient(90deg, #f00 0%, #00f 100%)` |
| **radial** | Colors radiating out from the center | `radial-gradient(ellipse at center, #f00 0%, #00f 100%)` |
| **conic** | Colors sweeping around a center point | `conic-gradient(from 0deg at center, red 0%, blue 100%)` |

## Color stops

Enter your colors one per line, or comma-separated. Each stop is a CSS color
optionally followed by a position:

- **Colors:** `#f00`, `#ff0000`, `#ff000080` (with alpha), named colors like
  `red`, or functions like `rgb(255, 0, 0)` and `hsl(210, 100%, 50%)`.
- **Positions:** add a percentage or length after the color — `#fff 10%`,
  `#000 90%`, `red 0px`. A bare number (`blue 100`) is read as a percentage.
- With **two or more colors and no positions**, evenly-spaced percentages are
  filled in automatically (`0%`, `50%`, `100%`, …).

At least two color stops are required.

## Angle and shape

- **Angle** (linear / conic) — degrees. For **linear**: `0` points up, `90`
  points right, `180` points down (the CSS default). For **conic**: the starting
  angle of the sweep.
- **Shape** (radial only) — `ellipse` (default) stretches to fit a rectangle;
  `circle` keeps it perfectly round.

## Color interpolation (modern color spaces)

By default colors blend in the sRGB space. Set the optional **Color space** field
to interpolate in a perceptually-uniform space for a smoother, more vivid blend —
the tool prepends an `in <space>` clause, e.g.
`linear-gradient(in oklch 90deg, …)`. Supported spaces include `srgb`,
`srgb-linear`, `oklab`, `oklch`, `lab`, `lch`, `hsl`, `hwb`, and `display-p3`.

For the polar spaces (`hsl`, `hwb`, `lch`, `oklch`) you can add a hue-rotation
method after the space — `oklch longer`, `hsl increasing` — choosing
`shorter`, `longer`, `increasing`, or `decreasing`. This is a recent CSS feature;
older browsers ignore the clause.

## Repeating gradients

Turn on **Repeating gradient** to tile the color stops across the element with
`repeating-linear-gradient` / `repeating-radial-gradient` /
`repeating-conic-gradient`. This is most useful with explicit pixel positions —
for example `#000 0px, #fff 10px` makes a striped pattern.

## Examples

| Type | Colors | Angle / Shape | Output |
| --- | --- | --- | --- |
| linear | `#ff0000, #0000ff` | 90 | `linear-gradient(90deg, #ff0000 0%, #0000ff 100%)` |
| linear | `red, green, blue` | 45 | `linear-gradient(45deg, red 0%, green 50%, blue 100%)` |
| radial | `#fff, #000` | circle | `radial-gradient(circle at center, #fff 0%, #000 100%)` |
| conic | `red, yellow, red` | 90 | `conic-gradient(from 90deg at center, red 0%, yellow 50%, red 100%)` |
| repeating | `#000 0px, #fff 10px` | 45 | `repeating-linear-gradient(45deg, #000 0px, #fff 10px)` |

## FAQ

**Is it free and private?** Yes — your colors never leave your device, and the
tool keeps working offline once the page has loaded.

**How do I use the output?** Copy the `background-image: …;` line into a CSS rule
for any element. You can also drop the inner `…-gradient(...)` part anywhere a
CSS `<image>` value is allowed.

**Can I mix automatic and manual positions?** If any stop has an explicit
position, the rest are left exactly as you wrote them. Only when *no* stop has a
position are even percentages filled in for all of them.

**Which color formats work?** Hex (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`),
named colors, `rgb()/rgba()`, and `hsl()/hsla()` — anything a browser accepts as
a CSS color.
