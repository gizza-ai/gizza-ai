# principal-component-analysis competitor analysis (2026-08-13)

Tool: `principal-component-analysis` — local PCA for a pasted numeric matrix, returning eigenvalues, explained variance, loadings and projected scores.

## Sources checked

| Source | Surface | Relevant observations |
| --- | --- | --- |
| Stats Unlock PCA Calculator | Browser calculator | Paste/input matrix, choose correlation vs covariance style analysis, shows eigenvalues, explained/cumulative variance, component loadings and score coordinates. Emphasises in-browser calculation. |
| CodingAce Advanced PCA Data Reduction Calculator | Browser calculator | Exposes mean-centering and standardization concepts, has sample data, computes covariance/correlation decomposition, variance shares and reduced coordinates. |
| CodingAce PCA Visualizer | Browser visualizer | Accepts pasted dataset or CSV upload, offers optional standardization, shows covariance calculation, eigenvectors, explained variance, scores and plots. |
| StatsCalculators PCA Calculator | Browser calculator | Markets comprehensive PCA results including eigenvalues, explained variance, scree-style visualisation, loadings and related diagnostics. |
| R / scikit-learn PCA workflows | Desktop/code reference | Common expectations include deterministic component counts, explained-variance ratios, transformed coordinates, access to means/scales, and arbitrary sign convention for eigenvectors. |

## Table-stakes requirements and decisions

| Competitor/table-stake capability | In gizza model? | Decision for this tool |
| --- | --- | --- |
| Paste a numeric matrix with optional header row | Yes | Implemented. `data` is a multiline field; first non-numeric row is used as variable names. |
| Accept common delimiters | Yes | Implemented: comma, tab, semicolon and whitespace delimiters. |
| Optional explicit variable labels | Yes | Implemented. `labels` overrides the detected header and labels loadings. |
| Correlation vs covariance PCA | Yes | Implemented as `scale=true` default for standardized/correlation PCA and `scale=false` for covariance PCA. Page uses a checkbox with default checked state. |
| Select number of retained components | Yes | Implemented. `components=0` keeps all; positive values keep the top N, clamped to available columns. |
| Eigenvalues, variance shares and cumulative variance | Yes | Implemented in text and JSON outputs. |
| Scree plot / elbow hint | Yes, text-safe | Implemented as a text bar chart in the formatted report; avoids adding charting dependencies. |
| Loadings table | Yes | Implemented with sign-stabilized eigenvectors so output is deterministic. |
| Scores / transformed coordinates | Yes | Implemented in text, JSON and CSV. Text truncates to the first 20 rows; CSV/JSON include every row. |
| Download/export tabular scores | Yes | Implemented via `format=csv`, and the generic text-page download link can save output. |
| Full structured output | Yes | Implemented as pretty JSON with means, standard deviations, thresholds, loadings and scores. |
| Sample datasets / presets | Yes | Implemented with example chips for body measurements, subject marks, covariance PCA and CSV scores. |
| CSV file upload | Partly out of model | Not implemented. Current generic pure text pages work best with paste/textarea input; file upload would need a separate page pattern. Pasted CSV covers the core use. |
| Interactive scatter/biplot charts | Out of model for this build | Not implemented. The generator can render text output reliably; adding chart interactivity would need custom JS/visual design beyond the standard tool surface. Scores are exported so users can plot externally. |
| Missing-value imputation / categorical encoding | Out of model for a deterministic PCA primitive | Not implemented. The tool rejects non-finite/missing cells and documents that users should clean or encode data first. |
| Very large matrix / sparse PCA / randomized SVD | Out of model | Not implemented. The pure Rust Jacobi decomposition targets browser-scale local data with a 100-variable cap, not sparse or approximate high-dimensional ML workflows. |

## UX/control choices

- `data`: multiline textarea with a realistic header-row placeholder.
- `labels`: optional text input with comma-separated placeholder.
- `components`: numeric text field, default `0`; bounds are enforced by the descriptor and core.
- `scale`: default-checked checkbox for standardized correlation-matrix PCA; unchecked state is covered by page tests.
- `format`: enum select with friendly labels for `text`, `json`, and `csv`.
- Preset chips cover the common competitor patterns: quick sample, top-N components, covariance PCA and CSV export.

## Verification implications

The page and CLI checks must assert actual PCA numbers, not only that output exists. Coverage should include:

- default standardized text report with eigenvalues/loadings/scores;
- non-default covariance path (`scale=false`);
- `format=json` and `format=csv` enum paths;
- `components` boundary (`100` accepted, `101` rejected);
- query-param deep link that pre-fills text, enum, integer and checkbox fields;
- generated CLI snippet has no TODO or branding strings.
