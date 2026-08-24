# color-code-extractor — competitor analysis (2026-08-23)

Scan run before completion, per the tool creation loop. Findings are paraphrased observations only.

## Tools reviewed

| # | Tool | Reachable | Shape |
|---|------|-----------|-------|
| 1 | Browser CSS color extractor utilities | yes | Paste CSS/text, list detected color codes |
| 2 | Palette-from-URL/CSS extractors | yes | Deduplicate colors and show swatches |
| 3 | Developer regex/color code finders | yes | Extract hex/rgb/hsl values from text |

## Table stakes observed → our decision

| Capability | Fit | Where it landed |
|---|---|---|
| Extract hex colors including shorthand | in-model | `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` scanner |
| Extract `rgb()`/`rgba()` and `hsl()`/`hsla()` | in-model | core parses legacy comma and modern slash syntax |
| Deduplicate equivalent colors | in-model | normalized RGBA identity, usage counts retained |
| Output normalized hex palette | in-model | default `list` + `color_format=hex` |
| Include usage counts | in-model | `include_counts` option for list/table/vars/svg |
| Sort by source order or frequency | in-model | `sort=first_seen|frequency|hue|lightness|alphabetical` |
| Export CSV/JSON | in-model | `output_format=csv|json` |
| Generate CSS/Sass variables | in-model | `css_vars`, `scss`, `less`, Tailwind map |
| Include swatch preview | in-model | `output_format=svg` swatch sheet |
| Filter neutrals | in-model | `exclude_grey`, `exclude_monochrome` |
| Extract from remote URL | out-of-model | no network fetch for arbitrary pages in this pure text tool |
| Resolve CSS variables/cascade | out-of-model | requires browser style computation; concrete literals only |

## Notes

The shipped tool focuses on deterministic extraction from pasted text with multiple export formats. It intentionally avoids remote fetching and computed-style resolution, which belong to a browser crawler rather than a pure sandboxed converter.
