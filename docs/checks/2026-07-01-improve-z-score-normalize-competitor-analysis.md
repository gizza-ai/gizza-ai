# z-score-normalize competitor analysis (2026-07-01)

## Scope

Tool: `z-score-normalize`

Goal: normalize a pasted list of numbers with browser-local WebAssembly. Primary jobs are feature scaling for statistics and ML preprocessing: z-score/standard score, min-max scaling, max-abs scaling, and robust median/IQR scaling.

## Competitors reviewed

1. Good Calculators — Data Scaler (`goodcalculators.com/data-scaler/`)
   - Offers multiple scaler choices including z-score, min-max, median/IQR, and max-abs.
   - Gap to close: more than just z-score/min-max; users expect the same common scaler family in one place.
   - In-model action: added max-abs and robust scaling alongside z-score and min-max.

2. Koshegio — Online Calculator for Data Normalization and Standardization (`koshegio.com/data-normalization`)
   - Focuses on quick paste-in normalization with z-score and min-max output.
   - Gap to close: clear paste-first UX and concise descriptions for non-specialists.
   - In-model action: page copy explains separators, method choice, and output fields.

3. scikit-learn preprocessing documentation — RobustScaler
   - RobustScaler centers on median and scales by the interquartile range so outliers have less effect.
   - Gap to close: robust scaling should be available when outlier-heavy inputs make mean/std-dev scaling misleading.
   - In-model action: added `robust` method with median/IQR metadata in the result.

4. GeeksforGeeks — Z-score normalization definition/examples
   - Explains the standard z-score transformation and expected mean/std-dev behavior.
   - Gap to close: educational page copy should state what the result means and which standard deviation convention is used.
   - In-model action: page content and descriptor document population vs. sample standard deviation.

5. ML preprocessing articles comparing MinMax, Standard, and Robust scalers
   - Common guidance compares standardization, bounded min-max scaling, and robust scaling for skewed/outlier-heavy data.
   - Gap to close: users need a short "which should I use" guide, not just raw output.
   - In-model action: added method-selection guidance to the tool page.

## In-model improvements shipped

- Z-score standardization with population default and optional sample standard deviation.
- Min-max scaling to the 0–1 range.
- Max-abs scaling to preserve sign in the −1..1 range.
- Robust median/IQR scaling for outlier-heavy data.
- Descriptor/schema, manifest, browser page, web wrapper, CLI, unit tests, and Playwright coverage aligned to the exposed methods.
- Original page copy with usage guidance, method comparison, and privacy note.

## Out-of-model / not built

- Multi-column CSV/dataframe preprocessing like full sklearn or pandas pipelines.
- File upload, charts, histograms, and downloadable CSV output.
- Server-side batch processing, saved projects, accounts, or API endpoints.

## Verification notes

The final verification matrix for this tool includes block cargo tests, wafer build, wasm-pack web build, generator, CLI smoke for z-score/max-abs/robust paths, and the Playwright page spec for z-score/min-max/max-abs/robust UI behavior.
