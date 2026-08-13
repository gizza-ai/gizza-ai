# histogram-bin-calculator — competitor analysis (2026-08-13)

Scan run for `blocks/histogram-bin-calculator`, per
`.claude/skills/create-next-tool/SKILL.md` step 4. Everything below is **paraphrased** — no
competitor copy, branding, or trademarks are reproduced, and no competitor asset is used.
The reference implementations (NumPy, R, matplotlib) are described from their documented,
publicly specified behaviour — the formulas, not their prose.

## Viability check (done first)

- **Duplicate scan.** `ls blocks/ | grep -iE 'histogram|bin|frequenc|distribut|stats|percentile'`
  → nearest neighbours are `descriptive-statistics` (n/mean/median/sd/quartiles for a pasted
  column, no binning), `percentile-calculator` (single quantile lookup), `frequency-counter`
  (counts of *repeated discrete tokens*, not numeric intervals) and `data-clusterer`
  (KMeans/DBSCAN over CSV columns). None of them computes bin-count rules or emits a
  binned frequency table with edges. `docs/tool-skiplist.txt` has no `histogram`/`bin`
  entry. **Not a duplicate.**
- **Model fit.** Every rule here is a closed-form expression over n, sd and IQR
  (`log2`, `cbrt`, `sqrt`), and binning is one pass over a sorted vector. Zero
  dependencies, no data files, identical results on wasm32-wasip1 (chat/CLI) and
  wasm32-unknown-unknown (page). **In model.**

## Competitors reviewed

| # | Tool | Shape | Notes |
|---|------|-------|-------|
| 1 | `numpy.histogram_bin_edges` / `numpy.histogram` | The de-facto reference API | `bins=` accepts an integer, an explicit edge array, or a rule name: `auto`, `fd`, `doane`, `scott`, `stone`, `rice`, `sturges`, `sqrt`. `auto` = the finer (max bin count) of Sturges and Freedman–Diaconis, falling back to Sturges when the IQR is 0. `range=(lo, hi)` fixes the span and *discards* values outside it. Intervals are half-open `[a, b)` with the last bin closed on both sides. `density=True` normalises to unit area. This is the behaviour users compare against, so it is the spec the tool matches. |
| 2 | R `hist()` + `nclass.Sturges` / `nclass.scott` / `nclass.FD` | The other reference | Same three named rules exposed as separate functions; `breaks` also accepts a count, a vector of edges, or a width function. `hist()` then *snaps* the suggested count to "pretty" edges (`pretty()`) rather than using the raw rule width — which is why R's bin count often differs from NumPy's for the same data. `right = TRUE` is R's default, i.e. `(a, b]` — the opposite of NumPy. Also emits counts, mids, and densities. |
| 3 | Hosted "how many bins" calculators (Omni-style / Statology-style single-purpose pages) | The direct SERP competition | Typical surface: paste or type values, pick one rule (usually square-root, Sturges or Rice), get **one** number back. Some show the implied bin width. Most stop there — no bin edges, no frequency table, no percentages, and no side-by-side comparison of the rules. Several only accept a sample size `n` rather than the data itself, so they cannot compute Scott or Freedman–Diaconis at all. |
| 4 | Spreadsheet histograms (Excel Data Analysis ToolPak / Google Sheets chart bins) | What most users actually use today | Input is a range plus a "bin range" (explicit upper edges) or an automatic count; Excel's charting histogram exposes bin width, bin count, and overflow/underflow bins. Output is a frequency table plus a chart. Right-closed intervals. No rule names, no bin-width formulas, no density. |
| 5 | matplotlib `pyplot.hist` / `seaborn.histplot` | The plotting layer everyone lands in next | Takes the same `bins=` rule names via NumPy, plus `binwidth=` (seaborn), `range=`, `cumulative=`, `density=`, and `stat=` (`count`/`frequency`/`density`/`probability`/`percent`). Confirms cumulative + density + percent are table stakes for a binning surface, not extras. |

## Table-stakes → decision

Every table-stake below ends in the descriptor **or** the out-of-model list; none is dropped
silently.

| Table-stake (seen in) | Fit | Where it landed |
|---|------|---|
| Paste the raw data, not just `n` | 1,2,4 | `numbers` (multiline) — newline/comma/space/tab/semicolon/pipe separated, 2–100,000 finite values |
| Sturges | 1,2,3 | `rule = sturges`, and always in the comparison table |
| Scott | 1,2 | `rule = scott` (`h = 3.49 × sd × n^(-1/3)`), with an explicit sd-is-0 fallback note |
| Freedman–Diaconis | 1,2 | `rule = freedman_diaconis` (`h = 2 × IQR × n^(-1/3)`), with an IQR-is-0 fallback note |
| Rice, square-root | 1,3 | `rule = rice`, `rule = sqrt` |
| NumPy-compatible `auto` | 1 | `rule = auto` (default) — `max(Sturges, FD)`, Sturges-only when IQR is 0 |
| Manual bin count | 1,2,4 | `rule = manual` + `bins` 1–1000, **slider** |
| Fixed histogram range | 1,5 | `range_min` / `range_max`; values outside are reported as excluded, matching NumPy's discard semantics, and surfaced in the "Counted x of n" line |
| "Pretty"/rounded edges | 2,4 | `nice_edges` — rounds the width up to a 1/2/2.5/5 × power-of-ten step and snaps the first edge down when the range start is automatic (the in-model half of R's `pretty()`) |
| Edge-inclusion convention | 1,2,4 | `right_closed` — `[a, b)` (NumPy, default) vs `(a, b]` (R/Excel); the report states which is in force |
| Frequency table with edges, counts, percent | 2,4 | The bin table: index, interval label, count, percent |
| Cumulative counts / cumulative % | 4,5 | `cumulative` |
| Density (unit-area height) | 1,2,5 | `density` — `count / (n × width)` |
| Visual shape | 4,5 | `chart` — one ASCII bar per bin in the report |
| Machine-readable export | 1,4 | `output` enum: `report` / `table` (TSV) / `csv` / `json` |
| Sample statistics beside the bins | 2,3 | Report header: n, min, max, range, mean, median, sd, Q1, Q3, IQR, 1.5×IQR outlier count, skewness, excess kurtosis |
| Side-by-side rule comparison | — | Not offered by any of 1–5 in one view; this is the tool's actual differentiator — every rule's bin count, implied width, and derivation printed together before one is applied |
| Precision control | 2 | `precision` 0–12, **slider** |
| Stated limits | — | Caps (2–100,000 values, ≤1,000 bins) are on the page, not just in code |

## Considered, not built (out of model or rejected)

- **Doane's and Stone's rules** (NumPy) — Doane is a defensible skew correction and Stone
  needs a leave-one-out cross-validation sweep over candidate bin counts. Both were weighed;
  five rules plus `auto` already cover what the SERP competition and the R/Excel users ask
  for, and adding two rules almost nobody names would dilute the comparison table that is
  this tool's whole point. *Considered, rejected* — not a model limitation.
- **A rendered SVG/PNG histogram** — the page driver renders one text or one media output,
  not a chart canvas (`references/page-patterns.md`). The ASCII bars carry the shape; `csv`
  carries the table into a real plotting tool.
- **Explicit non-uniform bin edges** (NumPy's array form, Excel's bin range) — schema bloat
  for the 1% case, and the resulting "bin width" column would be meaningless. `range_min` /
  `range_max` plus a manual count cover the practical version of this.
- **Overflow / underflow bins** (Excel) — gizza's equivalent is honest exclusion reporting:
  out-of-range values are counted and named in a note rather than silently folded into the
  end bins.
- **Weighted / frequency-paired input (`value, weight`)** — a real feature of grouped-data
  workflows, but it changes every statistic in the header (weighted sd, weighted quartiles)
  and the input parser. Out of scope for this pass; `frequency-counter` handles the discrete
  case upstream.
- **File upload of CSV/XLSX columns** — out of scope for a pure text block; gizza already
  ships `csv-column-extract` and `xlsx-to-csv`, so the composition is "extract → paste here".
- **Kernel density estimates / bandwidth selection** (seaborn) — a different tool
  (smoothing, not binning), and it needs a plot to be worth anything.
- **Cloud/batch datasets, accounts, saved projects** — outside gizza's browser-local,
  no-account model by definition.
