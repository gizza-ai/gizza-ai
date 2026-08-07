# constant-column-dropper — competitor scan + design decisions (2026-08-07)

Scan run **before** implementation, per `/create-next-tool` step 4. Everything below is
**paraphrased**; no competitor copy, branding, or trademarks are reproduced.

## Search

One web search for the tool's function: *"remove constant zero-variance columns from CSV tool"*.
The result set is dominated by **library/idiom** answers rather than hosted web tools — this
capability ships as a feature of data-science libraries (scikit-learn, caret, pandas/dplyr) and
inside larger data-cleaning platforms, not as a standalone paste-a-CSV page. That is the market
gap this tool fills.

## Competitors skimmed (top 3 reachable)

### 1. scikit-learn `VarianceThreshold` (feature-selection transformer)

- **Params / defaults:** a single `threshold` (float, default `0.0`). Features whose variance is
  below the threshold are removed; the default removes exactly the zero-variance (constant) ones.
- **Worked example (paraphrased from the docs):** a 3×4 integer matrix whose first and last
  columns hold one repeated value each; the default transform returns only the two middle columns.
- **Behavior:** numeric only — variance is a statistical quantity, so string/categorical columns
  are out of scope and NaN handling is implicit. Raises when *no* feature survives the threshold.
- **UX pattern:** fit/transform plus a boolean support mask (`get_support()`), i.e. the library
  equivalent of "give me the report as well as the cleaned table".

### 2. caret `nearZeroVar` (R, zero- and near-zero-variance predictors)

- **Two-metric model:** the *frequency ratio* (count of the most prevalent value ÷ count of the
  second most prevalent) and the *percent unique* (distinct values ÷ rows × 100). A predictor is
  flagged near-zero-variance when the frequency ratio is above a cut **and** percent-unique is
  below a cut; a true zero-variance predictor has a single distinct value.
- **Reporting:** with metrics enabled it returns a per-column table — `freqRatio`,
  `percentUnique`, `zeroVar`, `nzv` — rather than only an index list. Strong precedent for a
  **per-column report/JSON mode** next to the cleaned table.
- **Take-away:** "constant" is the strict case of a **dominance** continuum; the near-constant
  case is a real, widely used table-stake.

### 3. "Efficiently removing zero-variance columns" (T-Tested benchmark write-up)

- **Definition used:** a zero-variance column has exactly one distinct value; the fastest
  implementation is `min(x) == max(x)`, ~5–7× faster than computing variance or counting uniques.
- **Type coverage:** the distinct-value formulation is explicitly praised for working on
  **non-numeric (character) columns and NA values without tweaks**, unlike the variance formula.
- **Take-away:** define constancy as *one distinct value*, not *variance == 0* — it generalizes to
  text CSVs, which is exactly our input domain.

Supporting idiom checked: pandas `DataFrame.nunique()` defaults `dropna=True`, i.e. missing values
are **not** counted as a distinct value — so "does an empty cell count as its own value?" is a real
option with two defensible answers, and must be user-selectable.

## Table-stakes extracted

| Table-stake | Source | Decision |
|---|---|---|
| Drop columns with exactly one distinct value | all three | **Built** — default behavior (`dominance = 100`) |
| Works on text, not just numbers | T-Tested | **Built** — distinct-value rule over raw cells |
| Near-zero-variance / dominance threshold | caret | **Built** — `dominance` percent slider, 50–100, default 100 |
| Missing values: distinct value vs skipped | pandas, caret | **Built** — `empty_cells = value \| ignore` |
| All-empty column is degenerate | pandas/caret | **Built** — always dropped, reported as "all cells are empty" |
| Per-column metrics report, not just the cleaned table | caret `saveMetrics` | **Built** — `output = report \| csv \| json` with distinct count, top value, top share |
| Nothing survives → clear failure, not silent empty output | scikit-learn raises | **Built** — `output=csv` errors with a message naming the count and pointing at report mode |
| Protect identifier/label columns from being dropped | data-cleaning platforms | **Built** — `keep` list (names or 1-based indices) |
| Case / whitespace insensitivity when comparing cells | data-cleaning platforms | **Built** — `ignore_case`, `ignore_whitespace` (both default on, matching the sibling `duplicate-column-detector`) |
| Non-comma delimiters | all CSV tools | **Built** — `delimiter` enum: comma/tab/semicolon/pipe |

## Classification

**In-model (built):** everything in the table above. Pure string/table compute, no I/O, runs in
wasm in the browser and in the CLI.

**Out-of-model (considered, not built):**

- File upload / multi-file batch and 100 MB+ streaming — this is a paste-sized page; large jobs
  belong in a script. Stated as a limit on the page.
- Fitting on a training split and applying the same column mask to a test split (scikit-learn's
  fit/transform split) — needs persisted state across two runs; a stateless page cannot.
- Variance-in-the-statistical-sense thresholds on numeric columns (e.g. "drop variance < 0.01") —
  a different, numeric-only tool; the distinct-value/dominance rule was chosen deliberately
  because it also covers text columns.
- Charts/profiling of the column distribution — belongs to `csv-stats`.

**Considered, rejected:**

- `kind = "tag-list"` pills for the `keep` field. Rejected: column names can legitimately contain
  commas, and the tag-list control splits on them; a plain field with a worked placeholder keeps
  every name expressible. (Same reasoning already recorded for bulk-pasted list fields.)
- A separate `near_constant` boolean on top of `dominance`. Rejected as redundant: one dominance
  percent expresses both modes (100 = strictly constant, <100 = near-constant), with fewer
  controls to explain.
- An `output_delimiter` param. Rejected: `csv-cleaner` already owns delimiter rewriting; this tool
  round-trips the input delimiter.

## Descriptor designed from the scan

`data` (required, multiline) · `header` · `delimiter` (enum) · `dominance` (number 50–100, slider,
default 100) · `empty_cells` (enum value|ignore) · `ignore_case` · `ignore_whitespace` · `keep` ·
`output` (enum report|csv|json).

Preset `[[example]]` chips ship for the three real jobs: report constant columns, emit the cleaned
CSV, and catch near-constant columns at 95% dominance.
