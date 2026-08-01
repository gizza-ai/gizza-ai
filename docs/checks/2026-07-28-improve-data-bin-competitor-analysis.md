# data-bin — competitor analysis (2026-07-28)

Scan of the "bin a numeric column into buckets" space. One WebSearch
("data binning tool online bucket numeric column equal-width quantile") plus a
skim of the three most relevant references. There is no single dominant
consumer web tool for this — the de-facto references are the pandas/Azure ML
binning components, so I profiled those as the capability baseline. Everything
below is paraphrased from public docs; **no copy, branding, or trademarks were
reproduced.**

## Competitors profiled

### 1. pandas `cut` / `qcut` (reference implementation)
- `cut` = equal-width intervals; `qcut` = quantile / equal-frequency intervals.
- Params: number of bins (int) OR explicit edge list; `labels` (custom labels,
  or `False` → integer bin index); `right` (right-closed `(a, b]`, default true);
  `include_lowest` (make the first interval left-inclusive); `precision`
  (decimals in auto interval labels, default 3); `duplicates='drop'` to merge
  duplicate quantile edges on skewed data.
- Default auto label is the interval string, e.g. `(0.0, 25.0]`.
- Worked example: `pd.cut(score, bins=[0,50,80,100], labels=['C','B','A'],
  include_lowest=True)` → `[0,50]`, `(50,80]`, `(80,100]`.

### 2. Azure ML "Group Data into Bins"
- Binning modes: **Quantiles** (equal-height), **Equal Width** (specify number
  of bins), **Custom Edges** (comma-separated ascending edge list; edge is the
  lower boundary; output is the 1-based bin index).
- **Output mode**: Append (add a new column), Inplace (replace values),
  ResultOnly.
- "Number of bins" applies to Quantiles + Equal Width.

### 3. SAS PROC HPBIN / Google ML crash course (concept refs)
- Reinforce the same two core methods (equal-width "bucket" vs equal-frequency
  "quantile"), and that quantile binning is preferred for skewed data because
  each bucket holds ~equal counts.

## Table-stakes parameters (tagged in-model / out-of-model)

| Param | Competitor | Decision |
| ----- | ---------- | -------- |
| Equal-width vs quantile method | pandas cut/qcut, Azure | **in-model** — `method` enum |
| Custom bin edges | pandas `bins=[...]`, Azure Custom Edges | **in-model** — `method=custom` + `edges` |
| Number of bins | all | **in-model** — `bins` (default 4 = quartiles) |
| Custom labels | pandas `labels=[...]` | **in-model** — `labels` |
| Integer bin index labels | pandas `labels=False`, Azure | **in-model** — `label_style=index` |
| Auto interval labels | pandas default | **in-model** — `label_style=range` |
| Right- vs left-closed intervals | pandas `right` | **in-model** — `right` bool (default true) |
| Interval-label precision | pandas `precision` (3) | **in-model** — `precision` (default 3) |
| Append vs replace column | Azure Output mode | **in-model** — `output` enum (append/replace) |
| Column selection by name/index | all | **in-model** — `column` |
| Delimiter (comma/tab/;/pipe) | table-stakes CSV | **in-model** — `delimiter` |
| Header row handling | table-stakes CSV | **in-model** — `header` |
| Duplicate quantile-edge handling | pandas `duplicates='drop'` | **in-model** — dedupe edges automatically |

### Out-of-model (considered, not built)
- **Entropy/MDL supervised binning** (Azure Studio classic) — needs a target
  label + a supervised algorithm; out of scope for a stateless CSV transform.
- **Save/apply a binning transform to a second dataset** (Azure Apply
  Transformation) — needs persisted server-side state / an account.
- **Quantile normalization output modes** (Percent / PQuantile / QuantileIndex
  rescaling) — that is a normalization concern already covered by the sibling
  `data-normalize` tool; binning here emits categorical labels, not rescaled
  numbers.
- **Multi-column binning in one pass** — Azure applies one rule to many columns;
  our tool bins one explicitly chosen column per run (compose by re-running),
  which keeps labels/edges unambiguous.

## UX control decisions
- `method`, `label_style`, `output`, `delimiter` → `Param::enumv` (`<select>`
  with friendly labels).
- `right`, `header` → boolean checkboxes (default checked).
- `bins`, `precision` → integer fields with placeholders.
- `[[example]]` preset chips: equal-width quartiles, quantile terciles with
  custom labels, and custom age-band edges — doubling as the page's worked
  examples (mirrors competitors shipping preset "grade"/"age band" recipes).
