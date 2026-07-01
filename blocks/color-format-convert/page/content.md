## About this tool

**Color converter** takes a color in any common notation and gives you every other
representation at once:

- **HEX** (`#3498db`)
- **RGB / RGBA** (`rgb(52, 152, 219)`)
- **HSL** (`hsl(204, 70%, 53%)`)
- **HSV** (`hsv(204, 76%, 86%)`)
- **CMYK** (`cmyk(76%, 31%, 0%, 14%)`)

Paste a `#hex` (3, 4, 6, or 8 digits — 4/8 include alpha), an `rgb()` / `rgba()`,
or an `hsl()` / `hsla()` value and it parses it and converts to all the rest,
preserving the alpha channel.

### Privacy

Everything runs **in your browser** via WebAssembly. You can also run it from the
[gizza CLI](/) or inside a gizza chat (which return the values as structured JSON).

### Common uses

- Turn a designer's HEX into the RGB/HSL your CSS or code needs.
- Get CMYK for print from a screen color.
- Read an `rgba()` with alpha back as `#rrggbbaa`.

## FAQ

<details>
<summary>Can I paste an HSV or CMYK value as input?</summary>

Not currently — the parser accepts `#hex` (3, 4, 6, or 8 digits), `rgb()` /
`rgba()`, and `hsl()` / `hsla()`. HSV and CMYK appear in the output only. If
you have an HSV or CMYK color, convert it to one of the accepted notations
first.

</details>

<details>
<summary>Do I have to include the # before a hex code?</summary>

No. A bare value like `3498db` is recognized as hex as long as it contains
only hex digits. Components inside `rgb()` may be numbers or percentages, the
hue in `hsl()` may carry a `deg` suffix, and both comma and slash separators
are accepted — so CSS Level 4 syntax like `rgb(52 152 219 / 0.5)` parses too.

</details>

<details>
<summary>What happens to the alpha channel?</summary>

It is preserved through every conversion. A 4- or 8-digit hex carries alpha in
its last digit(s), and `rgba()` / `hsla()` accept alpha as `0–1` or a
percentage (values outside that range are clamped). The result shows it as
`#rrggbbaa`, in `rgba(...)`, and as a separate `alpha` value.

</details>

<details>
<summary>Is the CMYK value print-accurate?</summary>

It uses the standard mathematical RGB→CMYK formula, not an ICC color-managed
conversion — so treat it as a good starting point, and expect your print
shop's profile to shift the numbers slightly.

</details>
