# data-drift-summary — competitor analysis (2026-08-17)

Scan run BEFORE implementation, per `/improve-tool` Phase 2–3. All findings are **paraphrased**;
no competitor copy, branding, or trademarked wording is reused anywhere in the tool.

## Scope of the scan

"Compare two tabular datasets and report per-column drift" is dominated by Python libraries rather
than web tools — the real competitors are the report generators data teams run in a notebook or CI
job. Three were profiled in depth; a fourth is noted as an adjacent reference point.

| # | Competitor | Shape | Why it's the benchmark |
|---|---|---|---|
| 1 | Evidently (open-source, `evidentlyai.com`) | Python lib → HTML/JSON report | The reference implementation for "data drift report"; defines the default-method-by-type convention everyone else cites |
| 2 | ydata-profiling comparison report | Python lib → side-by-side HTML profile | The reference for *per-column profiling* (type, nulls, cardinality, range) of two dataset versions |
| 3 | Deepchecks train-test-validation suite | Python lib → HTML/notebook suite | The reference for *categorical* drift specifics — new categories, string-variant mismatch |
| — | whylogs / WhyLabs profile visualizer | Python lib → notebook widget | Adjacent; same feature envelope as 1+2, adds hosted monitoring (out of model) |

## 1. Evidently — Data Drift preset

- **Model:** two frames, `reference` vs `current`, compared column by column. Per-column drift score
  + a boolean "drifted", then a dataset-level verdict.
- **Table-stakes params observed:**
  - `method` / `stattest` — drift metric, applied to all columns or overridden per column
    (`num_method`, `cat_method`, `per_column_method`).
  - `threshold` — per-metric cutoff, also splittable into `num_threshold` / `cat_threshold`.
  - `drift_share` — share of drifted columns that flips the **dataset-level** verdict.
    **Default observed: 0.5.**
- **Defaults observed (method auto-selected by column type + row count):**
  | Column type | ≤ ~1 000 rows | > ~1 000 rows |
  |---|---|---|
  | Numerical | Kolmogorov–Smirnov, p-value cutoff **0.05** | Wasserstein distance, cutoff **0.1** |
  | Categorical (binary) | proportion Z-test, **0.05** | Jensen–Shannon distance, **0.1** |
  | Categorical (>2 levels) | Chi-square, **0.05** | Jensen–Shannon distance, **0.1** |
  | Text | domain-classifier tests (needs a model) | domain-classifier, **0.55** |
- **Threshold convention by metric family:** hypothesis tests (KS, chi-square, Z, Anderson, G-test,
  Mann–Whitney, t-test, TVD, …) all default to a **p-value of 0.05**; distance/divergence metrics
  (Wasserstein, PSI, Jensen–Shannon, KL, Hellinger, energy) all default to **0.1**.
- **UX:** summary banner ("dataset drift detected / not detected"), a sortable per-column table with
  the drift score, and expandable per-column distribution plots.

## 2. ydata-profiling — comparison report

- **Model:** profile dataset A, profile dataset B, `compare([a, b])` → one HTML report with every
  profile statistic shown **side by side** per column.
- **Table-stakes per-column facts:** inferred type, missing/null count and percentage, distinct
  count and distinct percentage, min/max/mean/median/std for numerics, most-frequent values for
  categoricals — all rendered as an A-vs-B pair.
- **Params observed:** per-dataset `title` (used as the column label for each side throughout the
  report), display `precision` (docs suggest a lower precision when comparing two profiles than for
  a single profile), and report styling/colors.
- **Stated use cases:** detecting drift between dataset versions, catching preprocessing errors, and
  confirming a train/test split has similar distributions.
- **Limits observed:** the compare path is 2+ pandas frames only; Spark frames are unsupported.
- **UX:** every stat is a labelled A | B pair — the naming of the two sides is a first-class,
  user-controlled thing, not "left/right".

## 3. Deepchecks — train-test-validation suite

- **Model:** a *suite of named checks* over a (train, test) pair, each with its own pass/fail
  condition, rather than one uniform drift number.
- **Checks directly relevant to a per-column drift summary:**
  - **Feature Drift** — a per-column drift score comparing the test distribution to the train
    distribution.
  - **New Category** — category values present in the test set but absent from the train set. This
    is shipped as a distinct, named *feature*, not as a footnote of the drift score.
  - **String Mismatch Comparison** — near-variants of the same category string that appear only on
    one side (their documented example: `New York` on one side vs `new york` on the other). Framed
    explicitly as an inference-time error-prevention check.
  - Multivariate drift (domain classifier), label drift, train/test sample mix.
- **UX:** per-check verdict + a short "why this matters" explanation, sorted worst-first.

## Table-stakes synthesis → in-model decisions

Every table-stake below is either implemented or explicitly listed as out-of-model — none dropped
silently (per `/new-tool` step 4).

| Table-stake (source) | Decision | How it lands |
|---|---|---|
| Two named sides, not "left/right" (2) | **In** | `reference` + `current` — the Evidently naming, which is also what the drift literature uses |
| Per-column inferred type, both sides (2) | **In** | `type` column shows `int`, `number`, `bool`, `date`, `string`, `empty`; a change renders as `int → string` and is flagged as schema drift |
| Per-column null/missing rate, both sides (2) | **In** | null % on each side + the delta; blank/whitespace **and** the literal tokens `NA`, `N/A`, `NULL`, `NaN`, `None` count as null (documented on the page) |
| Per-column distinct count / cardinality (2) | **In** | distinct count each side + delta |
| Numeric range/min/max (2) | **In** | `min–max` on each side for numeric columns |
| New categories present only in the new data (3) | **In** | listed per column, capped and with an `(+N more)` overflow marker |
| Missing categories that disappeared (3) | **In** | listed per column, same capping |
| Case/whitespace category variants (3, String Mismatch) | **In** | `ignore_case` folds case **and** trims whitespace before category comparison, so `New York` / `new york ` stop reading as a new category |
| A per-column drift **score** (1, 3) | **In** | PSI (default) or Jensen–Shannon distance, selected via `method` |
| Numeric drift via binning (1) | **In** | `bins` (default 10) — reference-derived bin edges, current data binned into the same edges |
| A drift **threshold** param (1) | **In** | `threshold`, default **0.2** (the standard PSI "significant shift" band; the page documents 0.1 = moderate, and that 0.1 is the usual cutoff for Jensen–Shannon) |
| Dataset-level verdict from a share of drifted columns (1) | **In** | `drift_share`, default **0.5** — matching the observed convention |
| Categorical vs numerical routing by column type (1) | **In** | automatic: numeric columns with more than `max_categories` distinct values are binned; everything else is treated as categorical |
| Column subset / focus (1, per-column overrides) | **In** | `columns` — comma-separated allow-list, empty = every common column |
| Worst-first ordering (1, 3) | **In** | `sort` = `drift` (default, highest score first), `name`, or `order` (input column order) |
| Schema drift: added/removed columns (1, 2) | **In** | reported in the summary and in a dedicated section |
| Machine-readable output (1, JSON report) | **In** | `format` = `table`, `markdown`, `json`, `csv` |
| Delimiter / header handling (family invariant) | **In** | `delimiter` enum + `header` boolean, matching the sibling `csv-*` blocks |
| KS / chi-square / Z-test p-values (1) | **Considered, rejected** | An exact KS or chi-square *p-value* needs the Kolmogorov and incomplete-gamma distributions; shipping a hand-rolled approximation would produce numbers that look authoritative and disagree with SciPy at the third digit. PSI and Jensen–Shannon are exact, closed-form, and are themselves defaults in competitor 1 for anything above ~1 000 rows. Documented on the page. |
| Distribution plots / histograms per column (1, 2) | **Out of model** | This tool's surfaces are text/JSON/CSV; an interactive HTML plot report is a different product |
| Multivariate / domain-classifier drift, text drift (1, 3) | **Out of model** | Needs a trained model; gizza is pure Rust + ffmpeg, no ML runtime |
| Label / prediction / model-performance drift (1, 3) | **Out of model** | Requires model outputs, not just two tables |
| Hosted monitoring, dashboards, drift-over-time series (1, whylogs) | **Out of model** | Needs a backend and an account; gizza is browser-local and stateless |
| Spark / out-of-core datasets (2 explicitly lacks this too) | **Out of model** | Both inputs are pasted text bounded by browser memory; stated on the page |
| Per-column method/threshold override dictionaries (1) | **Considered, rejected** | A `{"col": {"method": …}}` map is schema bloat for a paste-two-tables tool; `columns` + a global `method`/`threshold` covers the same intent by re-running on a focused subset |

## Copy / SEO angles observed (ideas only, original wording written for our page)

- All three lead with the *decision* the report supports ("did the data change enough to care"),
  not with the statistic — our hero and intro do the same in our own words.
- All three explain what a drift score means in bands rather than as a raw number; our page
  documents the PSI bands and what each one implies.
- Competitor 3's framing of new categories as an *inference-time failure* (not a curiosity) is the
  clearest justification for surfacing them; our FAQ makes the same point in original wording.
- None of the three runs in a browser without installing Python — "paste two CSVs, nothing leaves
  the browser, no install" is our genuine differentiator and the page's SEO angle.

## Preset chips shipped (competitors ship notebook examples; chips are our declarative equivalent)

1. **Null-rate + range drift** — a numeric column whose nulls and range both move.
2. **New & missing categories** — a categorical column gaining and losing levels.
3. **Schema drift (type change + new column)** — a column that flips type and a column added.
4. **JSON report** — the same comparison rendered as structured JSON.
