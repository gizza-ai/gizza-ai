## About this tool

**Color Code Extractor** scans pasted CSS, HTML, JavaScript theme files, JSON
design-token dumps or plain text and returns one deduplicated palette. It
recognizes `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()`,
`hsl()`/`hsla()`, `hwb()`, named CSS colors and `transparent`.

Deduplication is by the actual color, not by spelling. For example `#f00`,
`#FF0000`, `red` and `rgb(255, 0, 0)` all become one `#ff0000` palette entry
with a usage count. Alpha is part of the identity, so `#ff0000` and
`rgba(255, 0, 0, .5)` stay separate.

### Worked example

Input:

```css
.a { color: #f00; }
.b { color: #FF0000; }
.c { color: red; }
.d { color: rgb(255, 0, 0); }
.e { background: hsl(210 50% 40%); }
```

Default output is a plain list with counts:

```text
#ff0000  ×4
#336699  ×1
```

Switch **Output format** to `css_vars`, `scss`, `less`, `tailwind`, `csv`,
`json` or `svg` when you need a reusable palette artifact instead of a plain
list.

### Options

- **Output format** chooses a plain list, CSV, JSON, CSS custom properties,
  SCSS/Less variables, a Tailwind colors map or an SVG swatch sheet.
- **Color notation** rewrites each entry as hex, RGB, HSL, HWB, first original
  spelling or an exact CSS keyword when one exists.
- **Sort palette** can keep source order, put frequent colors first, or sort by
  hue, lightness or rendered value.
- **Include usage counts** annotates list, CSV, variable and SVG outputs with
  how often each color appeared.
- **Include named CSS colors** is useful for stylesheets. Turn it off for prose
  so ordinary words like “orange” or “plum” are not treated as colors.
- **Exclude greys** drops neutral greys but keeps black and white.
- **Exclude all monochrome colors** drops black, white and every grey.
- **Uppercase hex digits** writes `#AABBCC` instead of `#aabbcc`.
- **Limit after sorting** keeps the top N colors; `sort=frequency` and a small
  limit is a quick way to find the dominant palette in a large stylesheet.
- **Variable prefix** controls generated names such as `--brand-1`, `$brand-1`
  or `brand-1`.

### Limits and edge cases

- Input is limited to **5 MB**.
- Class names, IDs and variables such as `.red`, `#header {`, `$blue`, `@blue`
  and `--brand-red` are skipped.
- Colors that depend on `var()` or `calc()` cannot be resolved statically and
  are skipped.
- Named colors are matched only as standalone words, not inside longer
  identifiers.
- SVG output is a simple labeled swatch sheet; it is not a full design-token
  system.

Everything runs locally in WebAssembly in your browser; your stylesheet is not
uploaded.

## FAQ

<details>
<summary>Why did several different spellings collapse into one color?</summary>

The palette is deduplicated by normalized RGBA channels. That means `#f00`, `red` and `rgb(255, 0, 0)` represent the same opaque red and become one entry. The usage count and JSON `spellings` field still preserve how many forms were seen.

</details>

<details>
<summary>Can this extract colors from minified CSS?</summary>

Yes. The scanner works on raw text and does not need formatting or line breaks. Paste a whole minified stylesheet and use **Sort palette = Most frequent first** plus a limit to find dominant colors quickly.

</details>

<details>
<summary>Should I include named CSS colors?</summary>

Leave **Include named CSS colors** on for CSS or design-token files. Turn it off when scanning prose-heavy HTML or text, where normal words such as “orange”, “snow” or “tan” could be accidental matches.

</details>

<details>
<summary>Why are CSS variables skipped?</summary>

A value like `color: var(--brand)` depends on definitions and cascade context outside the literal text. This tool extracts concrete color literals only; resolve variables first if you need their computed values.

</details>
