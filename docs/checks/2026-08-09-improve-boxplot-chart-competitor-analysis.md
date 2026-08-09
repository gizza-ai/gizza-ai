# boxplot-chart — competitor analysis (2026-08-09)

Scan run BEFORE implementing, per `/improve-tool` Phase 2–3. All findings are **paraphrased**
observations of what each tool exposes; no competitor copy, branding, or assets were reused.

## Competitors reviewed

| # | Tool | Shape |
|---|------|-------|
| 1 | StatsKingdom — advanced box plot maker | Paste columns, heavy statistical + styling option set, PNG/JPG/SVG export |
| 2 | boxplotcalculator.com | Paste one list, five-number-summary calculator + plot, separate multi-series page |
| 3 | graphmaker.org — box plot maker | Section-based data entry, styling/typography focus, PNG/JPEG/PDF/SVG export |
| — | Also surveyed from the SERP (not deep-read): Kanaries, Flourish, MakeCharts, imageonline graphmaker | Same feature envelope: paste/CSV in, auto quartiles + outliers, category split |

## Table stakes observed

| Capability | Seen in | Defaults observed | Verdict |
|---|---|---|---|
| Paste raw numbers separated by commas / spaces / newlines | 2, 3 | — | **in-model** → `layout=values` |
| Paste columns with headers, one column per group (wide) | 1, 3 | — | **in-model** → `layout=wide` |
| Long/tidy data split by a category column | Kanaries, Flourish | — | **in-model** → `layout=long` + `group_column`/`value_column` |
| Delimiter tolerance (comma, tab, space, semicolon), non-numeric/blank cells | 1 | auto | **in-model** → auto delimiter detection; blanks skipped, bad cells error with line number |
| Quartile method choice (inclusive / exclusive / interpolated) | 1 | linear-ish | **in-model** → `quartile_method = linear \| inclusive \| exclusive` |
| Five-number summary + IQR + n + outlier list as text | 2 | shown next to plot | **in-model** → `output = summary` (and `json`) |
| Tukey 1.5×IQR fences with outliers as points | 1, 2 | k = 1.5 | **in-model** → `whiskers=tukey`, `iqr_multiplier` default 1.5 |
| Whisker variants: min/max ("only whiskers"), percentile | 1, 2 | Tukey default | **in-model** → `whiskers = tukey \| minmax \| percentile` + `percentile` |
| "All points" / show every observation | 1 | off | **in-model** → `points = outliers \| all \| none` |
| Mean marker | 1, 2 | optional | **in-model** → `show_mean` (default on) |
| Notched boxes (median CI) | 2 | off | **in-model** → `notched` (median ± 1.58·IQR/√n) |
| Vertical / horizontal orientation | 1, 2 | vertical | **in-model** → `orientation` |
| Grid on/off | 1, 2 | on | **in-model** → `grid` |
| Chart title + axis labels | 1, 3 | blank | **in-model** → `title`, `value_label`, `group_label` |
| Color themes / series color | 1, 2, 3 | blue-ish | **in-model** → `color` (any CSS color) + `theme = light \| dark` |
| Chart dimensions | 1 (margins/size) | ~800×480 | **in-model** → `width`, `height` |
| SVG download | 1, 3 | — | **in-model** → output *is* an SVG; the page offers the .svg download |
| Two-tier outliers (mild k=1.5 vs extreme k=3) | 1 | — | **in-model, folded** into `iqr_multiplier` (set 3.0 for the stricter fence) rather than a second point class — one fence rule keeps the summary/JSON unambiguous |
| Log value scale | 1 | linear | **considered, rejected** — log axes need per-tick relabelling and break on ≤0 values; the family invariant here is one linear value axis. Revisit if asked for. |

## Out-of-model (considered, not built)

- PNG / JPG / PDF export (1, 3) — gizza blocks emit one deterministic text artifact; the SVG is
  vector and converts losslessly in any editor. No rasteriser in the pure-Rust/wasm model.
- Interactive tooltips, hover, drag-to-reorder, click-to-hide series (1, Flourish) — the page
  output is a static `<img>` data URI by design (SVG through `<img>` cannot execute script).
- Font family / font-size pickers and per-element color pickers (1, 3) — deep styling belongs to
  editing the emitted SVG; exposing a dozen style params would bloat the chat schema.
- File upload / cloud storage / account-saved charts (Flourish, Kanaries) — no accounts, no server.
- Post-generation interactive "exclude these outliers and rescale" toggle (1) — the equivalent is
  re-running with `whiskers=minmax` or a different `iqr_multiplier`; a stateful editor is out of
  the one-shot tool model.

## Gaps closed in this build

Every "in-model" row above ships in `descriptor()` at first release: 20 params covering three
input layouts, three quartile methods, three whisker rules, point display modes, mean marker,
notches, orientation, grid, title/axis labels, size, color, theme, and three output formats
(`svg`, `summary`, `json`).

## Notes

- Quartile semantics are pinned by unit tests on `[1..8]`: linear (R type 7 / `PERCENTILE.INC`)
  gives Q1 2.75 / Q3 6.25; inclusive and exclusive both give 2.5 / 6.5 for an even n, and differ
  on odd n (median kept in / excluded from each half). Competitors rarely document which rule they
  use — ours is stated on the page and in the param description.
- No competitor markup, copy, or styling was copied; the SVG renderer, page copy, and FAQ are
  original.
