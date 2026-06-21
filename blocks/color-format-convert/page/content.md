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
