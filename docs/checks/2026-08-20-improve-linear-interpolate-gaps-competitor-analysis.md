# linear-interpolate-gaps competitor analysis (2026-08-20)

## Sources scanned

- Pandas/DataFrame interpolation guides and examples: establish the common `method=linear`, limit/max-gap, direction, and edge-area vocabulary for filling NA runs in numeric series.
- R/zoo-style missing-value examples: emphasize a `maxgap` control so long runs stay unfilled instead of being silently invented.
- Online linear interpolation calculators: emphasize two-point / x,y interpolation and extrapolation, but generally focus on one missing value rather than bulk pasted series.

## Table-stakes mapped to this tool

| Capability / UX expectation | In model? | Decision in this tool |
| --- | --- | --- |
| Paste a plain numeric column with blanks or NA markers | Yes | `input` accepts lines, commas, semicolons, tabs, or spaces; built-in missing markers include blanks, `na`, `n/a`, `nan`, `null`, `none`, `nil`, `-`, `--`, and `?`. |
| Linear interpolation between nearest known neighbors | Yes | Core fills interior gaps on the straight line between surrounding anchors. |
| Respect real x spacing for time/timestamp/index columns | Yes | `layout=xy` forces x,y rows; `layout=auto` detects consistent two-column data. |
| Limit long missing runs | Yes | `max_gap` mirrors maxgap/limit controls; `0` means no cap. |
| Choose fill direction for capped runs | Yes | `direction=both|forward|backward` covers the common both/forward/backward choices. |
| Decide what happens before the first known value and after the last | Yes | `edges=leave|hold|extrapolate` separates strict interpolation from hold/extrapolate behavior. |
| Custom missing-value sentinels | Yes | `na_tokens` lets logger-specific markers like `-999` be treated as gaps. |
| Rounding / display precision | Yes | `decimals` affects computed values only, preserving known values verbatim. |
| Audit which values were filled | Yes | `output=csv` adds status per row; `output=json` adds counts and per-gap reports. |
| Spline, polynomial, seasonal, model-based imputation | No | Out of model for this pure Rust block; listed in page limits rather than built. |
| Date parsing and calendar-aware interpolation | No | Users can convert dates/timestamps to numeric x values first; parsing localized date formats would add ambiguity outside the current model. |
| Multi-column dataframe interpolation with grouped columns | No | This tool intentionally handles one y series or one x,y series. Broader dataframe operations belong in a separate tabular transform. |

## UX controls implemented

- Multiline textarea for the pasted series.
- Select controls for `layout`, `direction`, `edges`, and `output` with human labels.
- Slider controls for `max_gap` and `decimals`, matching the reference tools' bounded numeric controls.
- Tag-list input for extra missing markers.
- Preset examples for evenly spaced values, uneven x,y spacing, capped gaps, edge holding, and custom sentinels.

## Verification focus

The page/CLI checks must cover exact linear output, x,y spacing, max-gap boundary behavior, non-default direction, edge hold/extrapolate, custom missing tokens, CSV/JSON audit formats, and a query-string deep link.
