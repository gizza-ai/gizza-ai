# csv-sample — competitor analysis (2026-07-18)

Function: take a random, stratified, or top/bottom-N sample of rows from a CSV. Competitor scan
done BEFORE implementing. All notes are paraphrased — no competitor copy/branding/trademarks used.

## Competitors scanned (top 3 real tools)

1. **subsample** (paulgb, CLI) — randomly sample lines from CSV/TSV.
   - Options: fixed count (`-n`), decimal fraction (`-f`, e.g. 0.15), percentage (`-p`), retain
     header row(s) (`-r`), reproducibility seed (`-s`), algorithm choice (reservoir default /
     approximate / two-pass). Default = reservoir sampling, fixed size, random output order.
2. **statscalculators — Statistical Sampling Tool** (web, pandas/numpy) — import data, pick a
   sampling method, export sampled CSV. Methods: simple random, stratified (groups sampled
   separately), systematic (regular interval after random start), cluster, weighted, bootstrap.
   Inputs: data import, desired sample size, categorical variable for grouping. Defaults/seed not
   documented publicly.
3. **Epitools — Random sampling from a sampling frame** (web) — for stratified sampling you enter
   the column to stratify on; supports sub-group selection by column + comparison condition.
   (Fourth reference skimmed: strample — quantile/stratified sampler on the first numeric column.)

## Table-stakes params → decision

| table-stake | source | decision |
| --- | --- | --- |
| fixed sample size N | subsample `-n`, statscalc | IN-MODEL → `n` (integer, default 10) |
| sample by percentage/fraction | subsample `-p`/`-f` | IN-MODEL → `percent` (number, 0 = use n) |
| keep header row | subsample `-r` | IN-MODEL → `header` (boolean, default true) |
| reproducibility seed | subsample `-s` | IN-MODEL → `seed` (integer, default 42; seeded PRNG, no OS RNG) |
| stratified by a column | statscalc, epitools, strample | IN-MODEL → `method=stratified` + `stratify_column` |
| random sample | all | IN-MODEL → `method=random` (default) |
| top-N / bottom-N (head/tail) | common CLI (head/tail), backlog hint | IN-MODEL → `method=top` / `method=bottom` |
| delimiter (comma/tab/semicolon/pipe) | csv family norm | IN-MODEL → `delimiter` enumv |
| systematic sampling (every k-th) | statscalc | IN-MODEL → `method=systematic` |

## UX control patterns matched
- Enum dropdown for the sampling method (`Param::enumv` → `<select>`).
- Enum dropdown for delimiter.
- Preset "Try:" example chips (random N, top-N, stratified) — the declarative preset answer.
- Placeholders showing a real CSV + real sample size.

## Out-of-model (listed, not built)
- **Cluster / two-stage cluster / weighted / bootstrap sampling** — niche statistical methods
  beyond the "grab a representative subset of rows" job; would bloat the schema. Considered, not built.
- **Multiple output resamples / repeated draws** — the tool returns one sample per run; re-run with
  a different seed for another draw.
- File upload / cloud batch / accounts — out of the browser-local, no-server model.

## Determinism note
Random/stratified/systematic sampling is made reproducible with an in-house seeded PRNG (splitmix64),
so the page recompute-on-input model and tests are deterministic; change `seed` for a different draw.
