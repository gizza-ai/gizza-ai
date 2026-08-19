# iqr-outlier-trimmer — competitor analysis (2026-08-09)

Scan run **before** implementing, per `create-next-tool` step 4. All notes are paraphrased
observations of publicly visible behaviour; **no competitor copy, branding, or trademarks were
copied** into the tool. Out-of-model items are listed, not built.

## Duplicate check (done first)

`ls blocks/ | grep -iE 'outlier|iqr|quartile|stat|csv|data|filter|trim'` surfaced four candidates;
each was read before deciding to build:

| Existing block | What it does | Why this row is still distinct |
| --- | --- | --- |
| `blocks/outlier-detector` | Takes a flat **list of numbers** (space/comma/semicolon/newline separated) and **flags** outliers by z-score, modified z-score (MAD) and IQR; returns stats + flagged values with indices. | Reports; it never returns a dataset. No CSV/table input, no column selection, no row removal, no winsorizing. This row's deliverable is a **cleaned CSV** with rows dropped. |
| `blocks/csv-filter` | Keeps rows matching `<column> <op> <value>` (`== != < <= > >= contains`). | Needs the user to already know the numeric threshold. Tukey fences are **derived from the data** (Q1/Q3/IQR), which `csv-filter`'s grammar cannot express. |
| `blocks/data-normalize` | Rescales numeric CSV columns (min-max / z-score / robust median-IQR). | Rewrites every value; row count is unchanged. Uses the IQR as a *scale*, not as a *fence*. |
| `blocks/csv-stats`, `blocks/descriptive-stats` | Per-column / per-list summary statistics. | Reporting only; no filtered output. |

**Verdict: not a duplicate — build it.** The gap is "compute Tukey fences per column, then act on the
rows", which no existing block covers.

## Competitors reviewed

1. **Outlier Calculator (calculator.goldsupplier.com/outlier-calculator/)** — comma-separated list
   input, numeric keypad, three one-click preset datasets ("obvious outlier" / "no outliers" /
   "two outliers"). Outputs Q1, Q3, IQR, lower/upper fence, the outlier list split into *mild*
   (k = 1.5) vs *extreme* (k = 3), a **cleaned dataset** with outliers removed, a step-by-step
   solution tab and a box-plot tab. The 1.5/3.0 multipliers are fixed — no adjustable k.
2. **Outlier Calculator (miniwebtool.com/outlier-calculator/)** — list input (commas, spaces or
   line breaks), minimum 4 data points, negatives + decimals accepted, fixed k = 1.5. Outputs Q1,
   Q2/median, Q3, IQR, both fences, outlier values, outlier count **and percentage**, total/normal
   value counts, a box plot and a step-by-step breakdown. Explicitly documents its quartile
   convention: the **Moore & McCabe exclusive** method (TI-83/TI-85).
3. **IQR Calculator (agentsfordata.com/statistics/interquartile-range)** — editable data table
   **or CSV upload**, plus a **"Select Data Column" dropdown** to choose which column to analyse.
   Outputs Q1, Q3, IQR and the flagged unusual values with a distribution chart. Its quartile
   convention is not stated.
4. **`outlier-remover-csv` (PyPI)** — the closest functional match: `outlier(input_csv, output_csv,
   method)` with `method` ∈ `z_score` | `iqr` (IQR is the default). Reads a CSV, **removes the
   outlier rows**, writes a CSV. No column selector, no adjustable multiplier, no report.
5. **Kanaries RATH + the Tukey-fences workflow it documents
   (docs.kanaries.net/articles/detect-outlier)** — one-click detection (Isolation Forest) over an
   uploaded CSV/Excel file, box plots, z-score (±2.5) and Tukey fences (Q1 − 1.5·IQR /
   Q3 + 1.5·IQR). Its row-removal step is the R/pandas idiom
   (`dataset[!(dataset$col %in% outliers), ]`), and it explicitly presents **winsorizing** (replace
   an extreme value with the nearest in-fence value) and log transforms as alternatives to dropping
   rows.
   *(PlotNerd's IQR Outlier Detector and GetZenQuery's Outlier Calculator both returned HTTP 403 to
   the fetcher; the two calculators above replaced them so the scan still covers 5 reachable tools.
   Their search-result descriptions — client-side processing, Tukey + MAD, CSV/PNG/SVG export,
   markdown-table copy — matched the same table stakes.)*

## Table stakes → decisions

| # | Capability seen at competitors | Fit | Where it landed |
| --- | --- | --- | --- |
| 1 | CSV in / CSV out with the header row preserved | in-model | `input`, `header`, `delimiter` params; the header row is copied verbatim and never fence-tested |
| 2 | Choose which column to analyse (dropdown at #3, absent at #4) | in-model | `columns` — comma-separated header names **or** 1-based indexes; blank = every numeric column (same convention as `blocks/data-normalize`) |
| 3 | Remove the outlier rows | in-model | `action = "remove"` (default) |
| 4 | Adjustable fence multiplier (all five hardcode 1.5, or 1.5/3.0) | in-model — **we beat them here** | `k`, default 1.5, rendered as a **slider** (0–5, step 0.1) plus preset chips for the mild (1.5) and extreme (3.0) conventions |
| 5 | Report Q1 / Q3 / IQR / lower fence / upper fence, outlier count **and percentage** | in-model | `output = "report"` emits exactly these per column plus the kept/removed row counts and percentage; `output = "csv"` (default) stays a clean, pasteable CSV with no comment pollution |
| 6 | Mild (1.5) vs extreme (3.0) outlier distinction | in-model | covered by #4's preset chips rather than a separate classification column — same two numbers, one control |
| 7 | Winsorizing / capping instead of dropping (#5 calls this out explicitly) | in-model | `action = "clip"` — clamps each selected cell to its own fence, row count unchanged |
| 8 | Inspect *what* was removed before committing to it | in-model | `action = "keep"` returns only the outlier rows (the inverse selection); `action = "flag"` appends an `outlier` column of `true`/`false` and drops nothing |
| 9 | A stated quartile convention (#2 is Moore & McCabe/TI-83; pandas/numpy use linear interpolation; Excel ships both) | in-model — **we beat them here** | `quartile_method` = `linear` (default, numpy/pandas/`QUARTILE.INC`) \| `exclusive` (Moore & McCabe, TI-83, `QUARTILE.EXC`-style) \| `inclusive` (Tukey's hinges). Every competitor hardcodes exactly one, and only one of them says which. |
| 10 | Multi-column datasets | in-model | multiple columns may be selected; `match` = `any` (default, drop a row if any selected column is out of fence) \| `all` |
| 11 | Non-numeric / blank cells in the analysed column | in-model | `non_numeric` = `keep` (default) \| `remove`; blank and unparseable cells are excluded from the quartile maths either way |
| 12 | Preset example datasets (#1 ships three) | in-model | four `[[example]]` chips: default trim, extreme k = 3, winsorize, and stats report |
| 13 | Copy / download the cleaned data | in-model, already platform-provided | `format = "text"` pages get Copy + Download for free from the generator |
| 14 | Runs locally, nothing uploaded | in-model | the whole block is wasm; stated in the page copy |
| 15 | Minimum data-point guard (#2 requires ≥ 4) | in-model | a column needs ≥ 2 numeric values for quartiles; with fewer, the fences are undefined so no row is trimmed on it — documented in Limits |

## Out-of-model (listed, not built)

- **Box-plot / distribution charts** (#1, #2, #3, #5). Blocks return text; charting belongs to a
  visual tool — `blocks/boxplot-chart` already renders box plots and pairs with this one.
- **Step-by-step solution walkthrough** (#1, #2). Teaching output, not a data transform; the
  `report` output gives the same numbers without the prose.
- **Z-score / modified-z-score / Isolation-Forest detection** (#4, #5). Deliberately out of scope:
  `blocks/outlier-detector` already ships z-score + MAD, and Isolation Forest needs an ML model
  (out of the pure-Rust + ffmpeg model).
- **File upload / drag-and-drop of .csv/.xlsx** (#3, #5). Pages take pasted text; `blocks/xlsx-to-csv`
  handles spreadsheet conversion upstream.
- **PNG/SVG export and markdown-table copy** (PlotNerd). Rendering concerns outside a CSV→CSV block.
- **Log transformation** as an outlier remedy (#5). A different operation (rescaling, not fencing) —
  closer to `blocks/data-normalize`.

## Verification plan

Core unit tests (happy + error + every enum value + cap boundary), the schema drift guard,
`gizza tool iqr-outlier-trimmer …` with one exact-output case, and a Playwright spec covering the
default trim, one real run per `action`, per `quartile_method` and per `match` value, a non-default
`header` checkbox state, the exact row cap and one-over, and a `?param=` deep link.
