# descriptive-stats — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/descriptive-stats` — sum, mean, median, mode, variance, std
dev, quartiles, min, max for a list of numbers. Pure-Rust, dependency-free.
Pure-text input → text/data output: chat + CLI + a page. (csv-stats summarises CSV
columns; this takes a flat number list.)

## What competitors do

- **`pandas Series.describe()` / R / Excel** — the standard, but need an
  environment or a spreadsheet and per-measure formulas.
- **Online "statistics calculator" sites** — quick, but the data is uploaded and
  measures/quartile methods are inconsistent.
- **Calculators** — fine for tiny lists, painful for many values.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The numbers never leave the device.
2. **Comprehensive in one call.** count, sum, mean, median, **mode (incl.
   multimodal, or none when all unique)**, min, max, range, **Q1/Q3/IQR**, and
   **both population and sample** variance & standard deviation — most quick tools
   give only a subset or are vague about population vs sample.
3. **Well-defined quartiles.** Linear-interpolation percentiles (the numpy default),
   documented — so results are reproducible.
4. **Forgiving input.** Numbers separated by spaces, commas, semicolons, or
   newlines; clear error naming the first non-numeric token.
5. **Structured + same everywhere.** Chat/CLI return a JSON object (each measure a
   field); the page shows a readable list.

## Honest scope

- **Univariate descriptive stats** — not inferential stats (confidence intervals,
  hypothesis tests), histograms, or correlation (see correlation-heatmap).

## Tests

7 core unit tests: the classic `2,4,4,4,5,5,7,9` set (mean 5, mode 4, pop variance
4 / std 2, sample variance 32/7); median for even/odd counts; numpy-linear
quartiles on `1..5` (Q1 2, Q3 4); no mode when all unique; **multimodal** detection;
single value → no sample variance; and errors (empty, non-numeric token). Plus the
block drift-guard schema test. **CLI verified** end-to-end. **Page** verified with
Playwright. `wafer build` instantiates the chat block (322 KiB).
