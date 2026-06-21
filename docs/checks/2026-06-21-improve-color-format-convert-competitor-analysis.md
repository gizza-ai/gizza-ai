# color-format-convert — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/color-format-convert` — convert a color between HEX, RGB(A),
HSL, HSV and CMYK. Pure-Rust, dependency-free. Pure-text input → text/data output:
chat + CLI + a page.

## What competitors do

- **Online color converters / pickers** (many) — paste a value, see conversions.
  Plentiful and useful, but ad-heavy and you leave the page/app you're in.
- **Browser devtools color picker** — great in CSS, but only HEX/RGB/HSL and only
  while editing a style.
- **Design apps (Figma, Photoshop)** — show formats, but you need the app open and
  the color selected.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page — convert a color without leaving your terminal or chat.
2. **One input → all formats.** Auto-detects `#hex` (3/4/6/8 digits), `rgb()`,
   `rgba()`, `hsl()`, `hsla()` and returns **HEX, RGB, RGBA, HSL, HSV, and CMYK**
   together, so you don't run several conversions.
3. **Alpha-aware.** 4/8-digit hex and `rgba()`/`hsla()` carry alpha through; the
   alpha is reported and round-trips to `#rrggbbaa`.
4. **CMYK included** — useful for print work, which many quick converters omit.
5. **Agent/CLI-friendly.** Chat/CLI return a structured object (each format a
   field) the model or a script can pick from; the page shows them all.

## Honest scope

- **Input formats: hex / rgb / hsl** (with alpha). Named CSS colors (`rebeccapurple`)
  and `lab()`/`lch()`/`oklch()` are not parsed.
- **CMYK is a simple device conversion** (no ICC color profile) — fine for quick
  reference, not color-managed print proofing.

## Tests

6 core unit tests: `#ff0000` → correct RGB/HSL/HSV/CMYK; short hex (`#0f0`) expands
and 8-digit hex carries alpha; `rgb()`/`rgba()` input (alpha preserved); `hsl()`
input round-trips to the right hex; mid-gray CMYK (`k=50%`, no chroma); and error
cases (empty, bad length, junk, too-few components). Plus the block drift-guard
schema test. **CLI verified** end-to-end. **Page** verified with Playwright.
`wafer build` instantiates the chat block.
