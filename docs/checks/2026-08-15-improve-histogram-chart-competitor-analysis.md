# histogram-chart — competitor analysis (2026-08-15)

Scan run **before** implementation, per `/improve-tool` Phase 2. All findings are paraphrased
observations of publicly visible feature lists. **No competitor copy, branding, or trademark text
was copied into this repo**; out-of-model items are listed here, not built.

## Scope + duplicate check

The backlog row is "Bin a list of numbers and render a histogram with auto or explicit bin counts."

Nearest existing blocks were checked before building:

- `blocks/histogram-bin-calculator` — a **bin-count-rule advisor**: it reports what Sturges /
  Scott / Freedman-Diaconis / Rice / sqrt each *recommend* side by side, then prints a text
  report with an ASCII bar column. Its product is a statistics report, not a chart.
- `blocks/data-bin` — labels **CSV rows** with the bucket each row falls in (adds a
  `<column>_bin` column). Its product is a transformed table, not a chart.
- `blocks/frequency-distribution` — tallies **symbols** (characters/bytes/n-grams), not numeric
  bins.

`histogram-chart` is the missing member of this repo's SVG chart family
(`scatter-chart`, `line-series-chart`, `boxplot-chart`, `hexbin-density-chart`,
`heatmap-chart`, `candlestick-chart`, `pie-donut-chart-svg`): numbers in, a standalone SVG image
out. Not a duplicate — built.

## Competitors reviewed

| # | Tool | What it offers (paraphrased) |
|---|------|------------------------------|
| 1 | histogrammaker.app | The deepest of the four. Binning rules FD (default), Scott, Sturges, Rice, Doane, square-root, plus custom bin count and custom bin width. Auto or custom min/max range, IQR-based outlier exclusion. Y-axis modes: count, relative frequency, density, cumulative count, cumulative percentage. Title, subtitle, x/y axis labels, caption. Mean marker, median marker, normal-curve overlay, rug plot. Eight visual themes, ten colour palettes, opacity. Exports PNG (many DPIs), SVG, PDF, CSV frequency table, JSON settings. Paste or CSV/TXT upload with multi-column selection. |
| 2 | makehistogram.com | Auto bins, Sturges, square-root; adjustable bin count and bin width; custom min/max range. Chart title and x/y axis labels. Frequency table showing frequency, relative frequency (percent) and cumulative frequency. Summary stats: count, mean, median, sd, min, max. Paste or CSV/TXT/XLSX upload. Exports PNG, SVG, CSV. Emphasises local, no-signup processing. |
| 3 | make-charts.com (histogram generator) | Toggle between a fixed bin count and an exact bin width; square-root rule as the stated default. Bar colour picker, gridline toggle, axis toggles. Value labels on bars showing counts *or* percentages, positioned top/centre/bottom. Optional stats panel (mean, median, sd, range). Exports PNG, SVG, share links; file upload and embeds are paid tiers. |
| 4 | chartload.com (histogram) | Deliberately minimal: auto-binning only, bar recolour, chart title, exports PNG/SVG/PDF. Explicitly exposes **no** bin-count, bin-width, range, normalisation, axis-label, cumulative, value-label or stats controls. Useful as the floor, not the bar. |

## Table stakes → decision

Every table-stake below lands in the descriptor or in the out-of-model list. Nothing dropped.

### In-model — built into the descriptor

| Table stake | Param(s) |
|---|---|
| Paste numbers in any common separator (newline/comma/tab/semicolon/space), tolerate a header row | `data` |
| Auto binning **and** every named rule the field uses | `bin_method` = `auto`, `sturges`, `scott`, `freedman_diaconis`, `rice`, `doane`, `sqrt`, `count`, `width` |
| Explicit bin count (the backlog row's "explicit bin counts") | `bins` (1–500, used by `bin_method=count`) |
| Explicit bin width, as a first-class alternative to count | `bin_width` (used by `bin_method=width`) |
| Custom min/max range instead of data min/max | `range_min`, `range_max` |
| Interval closure convention | `right_closed` (`[a,b)` default vs `(a,b]`) |
| Y-axis normalisation: count / relative / percent / density / cumulative | `normalize` = `count`, `relative`, `percent`, `density`, `cumulative_count`, `cumulative_percent` |
| Value labels on bars | `show_values` |
| Mean marker, median marker | `show_mean`, `show_median` |
| Normal-curve overlay | `normal_curve` |
| Rug plot | `rug` |
| Gridline toggle | `grid` |
| Chart title, x-axis label, y-axis label | `title`, `x_label`, `y_label` |
| Bar colour, opacity, light/dark theme | `color`, `opacity`, `theme` |
| Chart size | `width`, `height` |
| Frequency table export (bin ranges, counts, relative, cumulative) | `output` = `table` / `csv` |
| Machine-readable stats + bins | `output` = `json` |
| Summary stats (n, mean, median, sd, min, max, quartiles) | included in `table` and `json` output |

Two competitors ship preset buttons, so the page carries `[[example]]` preset chips, and the
bounded numeric params (`bins`, `bin_width` neighbours, `opacity`, `width`, `height`) use the
generator's `kind = "slider"` control with `kind = "color"` for `color`.

Beyond the four competitors, `orientation` (`vertical`/`horizontal`) and `precision` are added —
cheap in a deterministic SVG renderer and useful for long bin labels.

### Out-of-model — listed, not built

- **PNG / PDF / DPI-preset export.** This block's page surface renders `format = "text"` and its
  product is SVG markup, which any browser or converter rasterises. Bundling a rasteriser plus
  DPI/poster/slide export presets is a rendering-pipeline concern, not a compute one.
- **File upload (CSV / TXT / XLSX) with interactive column selection.** The page input is a
  paste field; XLSX in particular needs a spreadsheet reader. Pasting a column works today.
- **Named palette families (Viridis, Magma, Brewer, Okabe-Ito) and eight prebuilt themes.** A
  single-series histogram uses one bar colour; sequential palettes exist to encode a second
  dimension, which a histogram does not have. `color` + light/dark `theme` covers the need.
- **IQR-based outlier exclusion as a binning mode.** `range_min`/`range_max` already clip the
  plotted range explicitly and report how many values fell outside, which is the same outcome
  under user control rather than a hidden rule.
- **Share links, embed codes, AI-from-text-description generation, paid tiers.** Hosting and
  account features, out of scope for a local pure-Rust block.
- **Subtitle and caption text fields.** `title` plus the two axis labels cover the labelling need;
  more free-text slots would add descriptor surface without changing the computation.

## Verification plan

CLI exact-output case, advertised-value matrix across every `bin_method` / `normalize` / `output`
enum choice plus non-default checkbox states and the cap boundary, and a Playwright page spec
asserting real SVG content and a `?param=` deep link.
