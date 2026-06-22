# calendar-heatmap — competitor analysis & improvement check (2026-06-22)

## Tool
`calendar-heatmap` — turns a list of date→value pairs into a GitHub-style year
**contribution calendar** (an SVG): weeks are columns, the seven weekdays are rows,
and each day-cell is shaded by its value bucketed into a 5-step intensity scale, with
a `Less → More` legend and month/weekday labels. `data` is one date per line as
`YYYY-MM-DD` or `YYYY-MM-DD,VALUE` (value defaults to 1; repeated dates are summed).
Pure-Rust, no deps in core (hand-rolled Gregorian date math so it instantiates on all
backends). SVG image-bytes output → `build_media_envelope` (mime `image/svg+xml`).
Surfaces: **chat + CLI** (no standalone page — image-bytes output has no page render
mode, same as `heatmap-chart` / `correlation-heatmap` / `line-series-chart`).

## Distinct from existing blocks
- `blocks/heatmap-chart` renders an arbitrary numeric **M×N grid** (matrix) with a
  blue→yellow→red colormap. `calendar-heatmap` is the orthogonal layout: a fixed
  7-row weekday × weeks-per-year **calendar** keyed by date, GitHub-contribution-style.
- `blocks/correlation-heatmap` computes a symmetric correlation matrix; `blocks/table-heatmap`
  shades a data table; `blocks/animated-heatmap` is a spatial/animated heatmap. None take
  date→value pairs or produce a weekday×week calendar layout. This tool is a real, distinct
  visualization — not a near-dup — so it is built, not skiplisted.

## Competitors surveyed (top "calendar heatmap / GitHub contribution chart" tools)
1. **awajis.com Calendar Heatmap Generator** — upload a CSV of `date,value`
   (YYYY-MM-DD), GitHub-style calendar output for habit/activity tracking.
2. **ChartGen GitHub-style contribution graph** — activity intensity by day, habit/commit tracking.
3. **Cal-Heatmap (cal-heatmap.com)** — JS library; time-series calendar heatmap, CSV/JSON input,
   selectable color scales, tooltips, configurable date range/subdomain.
4. **DKirwan/calendar-heatmap (d3)** — d3 heatmap mirroring GitHub's contribution chart, hover tooltips.
5. **nikolaydubina/calendarheatmap (Go, calendarheatmap.io)** — JSON `{date: value}` in → PNG/SVG out,
   multiple colorscales, toggleable labels, month separators, locale month names.
6. **blurfx/calendar-heatmap (Go)** — SVG generator of a GitHub-style contribution calendar.

## Capability diff (competitor feature → our coverage)
| Capability | Competitors | calendar-heatmap | Notes |
|---|---|---|---|
| Date,value input (YYYY-MM-DD) | all | ✅ | `data`, one per line; `,`/space/tab separator |
| Value defaults to 1 (count mode) | several | ✅ | bare `YYYY-MM-DD` line → value 1 |
| Sum repeated dates | some | ✅ | duplicate dates accumulate |
| GitHub weekday×week layout | all | ✅ | 7 rows (Sun→Sat), full-week columns |
| 5-step intensity buckets | all | ✅ | quartile-of-max → level 0..4 |
| Multiple color schemes | Cal-Heatmap, dubina | ✅ | `scheme` = green/blue/purple/orange |
| Month labels | most | ✅ | drawn above the first column of each month |
| Weekday labels | most | ✅ | Mon/Wed/Fri (GitHub convention) |
| Less→More legend | GitHub-style | ✅ | bottom-right swatch row |
| Per-day tooltip | Cal-Heatmap, d3, GitHub | ✅ | SVG `<title>` `DATE: value` on every cell |
| Custom date window | Cal-Heatmap | ✅ | `start`/`end` override the auto data range |
| Title/heading | some | ✅ | `title` param |
| SVG output | dubina/blurfx | ✅ | `image/svg+xml`, scalable |
| PNG/JPEG export | dubina, awajis | out of model* | SVG is the vector source; rasterize downstream (see below) |
| Locale month names (i18n) | dubina | out of scope | English month abbreviations only |
| Tunable cell size / radius | Cal-Heatmap | out of scope | fixed GitHub-like 13px cell (kept faithful to the reference look) |

\* PNG export is intentionally not duplicated here: gizza already has `blocks/svg-to-png`,
so a user pipes this SVG through that tool — building a second rasterizer would be redundant.

## Gaps closed in this build
The first implementation already covers the full in-model competitor feature set:
date+value parsing with count-mode default and duplicate summing, the GitHub weekday×week
calendar layout, 5-step quartile buckets, **four** color schemes, month + weekday labels,
a Less→More legend, per-day `<title>` tooltips, an optional explicit date window
(`start`/`end`), and a title. No additional in-model capability gap remained after the
first pass.

## Out-of-model / intentionally NOT built
- **PNG/JPEG export** — SVG is the scalable source of truth; `blocks/svg-to-png` rasterizes it
  (avoids a redundant rasterizer in this tool).
- **No standalone page** — image-bytes (SVG) output has no page render mode in the page driver
  (same constraint as the other chart/heatmap blocks); chat + CLI are the supported surfaces.
- **Locale / i18n month names**, **tunable cell geometry** — kept faithful to the GitHub look;
  out of scope, no model limitation but deliberately not added to keep the schema small.

## Verification
- Unit tests (core, 11): weekday for known dates (incl. 1970-01-01 Thursday), leap-year Feb
  (2000 vs 1900), ordinal day-diffs, signed day-walk round-trip across year boundaries,
  quartile bucketing, basic render (title/darkest-bucket/legend/tooltip), duplicate-date
  summing, value-defaults-to-1, blue scheme, explicit window padding to full weeks, and the
  error set (empty / bad date / bad month / bad day / bad value / end-before-start) — all pass.
- Drift-guard schema test (block): authored chat schema == derived `schema_json()` — pass.
- `cargo test --workspace` — 12 tests pass (11 core + 1 block drift guard).
- `wafer build`: block.wasm validates & instantiates (354.3 KiB).
- CLI: `gizza tool calendar-heatmap data='…' scheme=blue title='2024 Activity'` → valid SVG
  (888×182 viewBox, blue darkest `#08519c` present, `2024-06-15: 9` tooltip, Less/More legend);
  single-date input and an invalid-date error case both behaved correctly.
- Page generator re-ran clean (209 tools); calendar-heatmap correctly contributes no page.

## Sources
- https://awajis.com/calendar-heatmap-generator/
- https://chartgen.ai/features/heatmap-generator
- https://cal-heatmap.com/v2/
- https://github.com/DKirwan/calendar-heatmap
- https://github.com/nikolaydubina/calendarheatmap
- https://github.com/blurfx/calendar-heatmap
- https://www.jqueryscript.net/blog/best-github-style-calendar-heatmap.html
