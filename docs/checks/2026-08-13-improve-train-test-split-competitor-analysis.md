# train-test-split — competitor analysis (2026-08-13)

Scan run BEFORE implementation, per `/improve-tool` Phase 2–3. All findings are **paraphrased**;
no competitor copy, branding, or trademarks are reproduced here or in the tool.

## Landscape

There is essentially **no dedicated browser-local GUI** for "split a CSV into train/test with
stratification". The category is owned by libraries and by dataset platforms:

| # | Competitor | Kind | What it is |
|---|-----------|------|------------|
| 1 | scikit-learn `train_test_split` / `StratifiedShuffleSplit` | Python library | The de-facto reference API everyone else copies |
| 2 | splitTools `partition()` (R, CRAN) | R library | One-call train/valid/test partition with stratified / grouped / blocked modes |
| 3 | caret `createDataPartition` (R) | R library | Stratified single split, seeded via `set.seed()` |
| 4 | Roboflow dataset split | Hosted GUI | Train/valid/test ratio picker on an uploaded dataset |
| 5 | Galaxy `sklearn_train_test_split` wrapper | Hosted GUI form | A web form over the sklearn call for bioinformatics workflows |
| 6 | H2O `split_frame` | Library/cluster | Ratio-vector split into 2–3 frames |
| 7 | `ttv` CLI, generic "CSV splitter" web tools | CLI / web | Chunk a big file; **not** ML-aware (no stratification, no seed) |

Generic online "CSV splitter" tools (file-splitter style) split by row count or file size only —
they are adjacent, not competitors: no class balance, no reproducible seed, no validation set.

## Table-stakes parameters (and where we landed)

| Table stake | Seen in | Our decision |
|---|---|---|
| `test_size` as a **fraction OR an absolute row count** | 1, 6 | `test_size`, default `0.2`; `<1` = proportion, `>=1` = row count |
| Third **validation** split in one pass | 2, 4, 6 | `validation_size`, default `0` (off) |
| **Reproducible seed** | 1 (`random_state`), 2, 3 | `seed`, default `42` — always set, so results are reproducible by default (sklearn defaults to `None` = non-reproducible) |
| **Shuffle** toggle | 1, 2 (`blocked`) | `shuffle`, default on; off = sequential/blocked split (train → validation → test in file order), the time-series-safe mode |
| **Stratify** on a label column | 1, 2, 3 | `stratify_column` (header name or 1-based index), largest-remainder proportional allocation |
| **Quantile binning** of a numeric stratify column | 2 (`n_bins`) | `stratify_bins`, default `0` (values used as categories); `>0` bins a numeric column into that many equal-count strata |
| **Grouped** split (keep a group's rows together — leakage guard) | 2 (`type="grouped"`), sklearn `GroupShuffleSplit` | `group_column` |
| `shuffle=False` ⇒ stratify not allowed | 1 | Same constraint, with an explicit error message |
| Header + delimiter handling | 7 | `header`, `delimiter` (comma/tab/semicolon/pipe) |
| Row counts / class-balance report | 2, 4 | `output = summary` prints per-split counts, percentages, and (when stratifying) the per-class balance in each split |
| Separate downloadable outputs | 4, 7 | `output = train/test/validation` emits one split alone; `output = json` emits all splits plus counts in one object |

## Defaults chosen (and why they differ)

- **`test_size = 0.2`** — sklearn's implicit default is `0.25`; the 80/20 convention is the one most
  written-down guidance recommends, and it is what tutorials, Roboflow-style pickers, and course
  material use. 0.2 it is, and the page states the ratio guidance (70/30 for small sets).
- **`seed = 42` always set** — sklearn's `random_state=None` makes every call non-reproducible by
  default, a classic footgun. A fixed default seed means the same input always yields the same split
  unless the user asks otherwise.
- **Original file order preserved inside each split** — competitors return shuffled rows. Keeping
  file order makes the output diffable and makes "did the right rows move" checkable by eye; the
  shuffle still governs *assignment*. Stated on the page.

## UX controls adopted

- `<select>` for `delimiter` and `output` (fixed vocabularies → `Param::enumv`).
- Checkboxes for `shuffle` / `header`, with a non-default state exercised in the page spec.
- Placeholders on every text/number field showing a real value (`0.2`, `label`, `patient_id`).
- Example chips for the recurring presets competitors ship: 80/20, 60/20/20 three-way, stratified by
  label, grouped, and a time-series (no-shuffle) split.
- Worked example on the page with both input and output, plus explicit limits/edge cases.

## Considered, NOT built (out of model or rejected)

- **k-fold / repeated CV fold generation** (splitTools `create_folds`, sklearn `KFold`) — a different
  tool shape (N outputs per run); out of scope for a train/test splitter.
- **Multi-file / zip download of each split as its own file** — this repo's page surface renders one
  text output; `output = json` covers programmatic multi-split consumption. (`csv-group-split` is the
  precedent for zip output, and it has no page.)
- **Stratified + grouped simultaneously** (sklearn `StratifiedGroupKFold`) — rejected: the greedy
  approximation is hard to explain and easy to misread. The tool errors with a message naming both
  params instead of silently doing something surprising.
- **Image/folder dataset splitting, cloud storage, accounts, out-of-core streaming for files that
  don't fit in memory** (`ttv`, Roboflow) — out of model: browser-local, no backend, no account.
- **Excel/Parquet input** — out of model for this block; the CSV-family tools handle conversion.
