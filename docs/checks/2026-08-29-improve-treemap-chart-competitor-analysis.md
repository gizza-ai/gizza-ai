# treemap-chart competitor analysis — 2026-08-29

Tool: `treemap-chart`
Backlog request: render a hierarchical treemap sized by value from nested or grouped data.

## Sources skimmed

1. Elysia Tools treemap generator (`elysiatools.com/en/tools/treemap-generator`)
   - Accepts a JSON hierarchy with names, children, and values.
   - Uses squarified treemap layout.
   - Controls observed: chart title, colour scheme, border width slider, show labels, show values, show percentages, prepared example run.
2. ChartLoad treemap maker (`chartload.com/charts/treemap/`)
   - Explains parent/child path rows such as `Parent > Child` plus positive numeric values.
   - Emphasizes hierarchy, category colour, labels, value/percentage tooltips, legend, templates, and common cases such as cloud spend, budgets, and disk-like composition.
   - UX expectations: examples/templates, hierarchy separator flexibility, readable labels, long-tail guidance.
3. RauGen treemap chart maker (`raugen.com/toolbox/treemap-maker`)
   - Accepts simple label/value rows with tab or multiple-space separation.
   - Controls observed: sample data, clear action, title, show labels, show values, show percentages, font-size slider, border-width slider, aspect ratio/dimensions, background/font/border colours, theme preset buttons, PNG download.

## Table-stakes mapped to this block

| Capability / UX pattern | In model? | Implemented decision |
| --- | --- | --- |
| Flat `label,value` data entry | yes | `data` textarea parses comma, tab, semicolon, and whitespace separated rows. |
| Hierarchical data | yes | `layout=path` splits the first column by `path_separator`; `layout=grouped` treats all but the last column as levels; `auto` detects common cases. |
| Squarified treemap | yes | Default `tiling=squarified`. |
| Alternate layout preserving order | yes | `tiling=slice_dice` and `tiling=binary` are exposed enum choices. |
| Sorting controls | yes | `sort=value_desc`, `value_asc`, `input`, or `label`. |
| Title | yes | `title` text drawn above the chart. |
| Labels, values, percentages | yes | `show_labels`, `show_values`, `show_percent`, plus `label_position`. |
| Palette/theme controls | yes | Six palette choices, `theme=light/dark`, mono base `color`, and background colour. |
| Sliders for visual tuning | yes | `font_size`, `border_width`, `corner_radius`, `width`, `height`, `max_depth`, and `top_n` use slider metadata on the page. |
| Legend | yes | `legend=true` draws top-level branch shares. |
| Examples / preset chips | yes | Four `[[example]]` chips cover flat storage, path rows, grouped data, and summary output. |
| Long-tail management | yes | `top_n` folds smaller siblings into `Other`; `max_depth` caps deep paths. |
| JSON / tabular output | yes | `output=json` for machine-readable geometry and `output=summary` for exact values. |
| PNG/JPEG export | out of model | This block is pure Rust and returns text/SVG; raster export is left to browser download/conversion outside the current model. |
| Hover tooltips/interactivity | partly out of model | SVG includes native `<title>` text per tile, but custom interactive hover UI belongs to the site runtime rather than the block. |
| Arbitrary JSON hierarchy input | intentionally not primary | The backlog calls for nested or grouped data. CSV/path/grouped rows cover the CLI/page model better than a JSON editor; JSON can be derived through `output=json`. |

## Verification focus

- Exact CLI summary output for a small dataset.
- SVG contains expected labels, values, and colour for default and mono short-hex runs.
- Page deep-link fills parameters and renders real SVG text.
- Enum matrix covers layout, sort, tiling, palette, theme, and output choices.
- Cap/error checks cover the documented 20,000-row input limit and invalid numeric rows.
