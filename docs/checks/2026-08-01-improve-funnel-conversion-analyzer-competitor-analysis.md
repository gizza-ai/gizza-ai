# funnel-conversion-analyzer — competitor analysis (2026-08-01)

**Tool:** Computes step-by-step conversion and drop-off rates from an event CSV. Pure Rust.

## Scan

One WebSearch ("funnel conversion analysis tool step drop-off rate calculation
methodology"). Skimmed the top real competitors / reference docs (paraphrased only —
no copy, branding or trademarks reused):

- **Count.co** — "Funnel Conversion Analysis" + "Drop-off Analysis" reference pages:
  define an ordered funnel of stages (e.g. landing → signup → trial → purchase),
  count users reaching each stage, report step-to-step conversion and the drop-off
  between consecutive stages.
- **Amplitude** (funnel-analysis / funnel-drop-off guides): map a user's path through
  a multi-step flow; measure conversion between each step; surface the biggest drop-off
  step; distinguish overall conversion (first step → last step) from step-to-step.
- **UXCam** (conversion-funnel-analysis / drop-off-rates): step-to-step vs. overall
  analysis; drop-off rate as the share of users lost between two consecutive steps.
- **MetricGate** (funnel drop-off calculator) and **Statsig** (measure funnel drop-off):
  the canonical formula and a per-step table of users / conversion% / drop-off%.

## Table-stakes → in-model / out-of-model

| Capability | Decision |
|---|---|
| Define an ordered funnel of steps | **in-model** — `steps` param (comma-separated); auto-derived from first-seen event order if blank |
| Count users reaching each step | **in-model** — unique users per step |
| Step-to-step conversion (vs. previous step) | **in-model** — `conversion_from_prev` |
| Overall conversion (first → last step) | **in-model** — `overall_conversion` |
| Drop-off count + drop-off rate per step | **in-model** — `dropoff` + `dropoff_rate`, canonical formula `(prev − current)/prev × 100` |
| Funnel semantics: require completing prior steps | **in-model** — `ordered` boolean (default true); off = count each step independently |
| Chronological ordering via an event timestamp | **in-model** — optional `time_column`; greedy in-order subsequence match over each user's time-sorted events |
| Configurable id / event / time columns | **in-model** — `user_column` / `event_column` / `time_column` by name or 1-based index |
| CSV delimiter + header handling | **in-model** — `delimiter`, `header` (matches sibling csv-* tools) |
| Table + JSON output | **in-model** — `format` enum (table \| json); chat/CLI return structured JSON |
| Segmentation / cohorts / breakdown by property | **out-of-model** — a full analytics engine feature; multi-dimensional pivoting is beyond a single-pass CSV tool (sibling `csv-pivot` / `csv-group-by` cover generic grouping) |
| Time-to-convert / conversion window | **out-of-model** — requires a windowed sessionization pass and richer time semantics than a single ordered pass; deferred |
| Charts / funnel visualization | **out-of-model** — the page output surface is text; `csv-chart-generator` covers charting |

Every table-stake landed in the descriptor except the three explicitly listed
out-of-model rows (segmentation, conversion-window, charts).

## UX / controls

Competitors ship preset funnels and a per-step results table. Ours mirrors that with:
`[[example]]` preset chips (a signup→purchase funnel, an independent-count example, a
timestamp-ordered example), a `format` `<select>` (table/json), an `ordered` checkbox,
and multiline CSV input. Output is a per-step table plus an overall-conversion line.
