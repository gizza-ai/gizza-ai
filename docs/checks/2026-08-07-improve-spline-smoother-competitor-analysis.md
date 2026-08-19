# spline-smoother — competitor analysis (2026-08-07)

Scan run **before** implementing, per `.claude/skills/create-tool-loop/SKILL.md` step 4.
All notes are paraphrased observations of publicly documented behaviour — no competitor copy,
branding or trademarks are reproduced, and out-of-model items are listed, not built.

## Search

One WebSearch: *"smoothing spline online tool fit noisy data smoothing parameter"*.
The field is dominated by library/reference implementations plus one hosted curve-fitting app,
so the top three reachable, real tools skimmed were:

| # | Tool | Reached | Role |
|---|------|---------|------|
| 1 | SciPy `make_smoothing_spline` / `make_splrep` / `UnivariateSpline` (docs) | yes | de-facto Python reference; GCV-selected penalty |
| 2 | MATLAB `csaps` (Curve Fitting Toolbox docs) | yes | de-facto `p ∈ [0,1]` smoothing-parameter convention |
| 3 | R `stats::smooth.spline` (R manual) | yes | de-facto `spar` / `df` / `cv` convention, reports effective df |
| 4 | SplineCloud curve-fitting (hosted web app) | yes | the closest *hosted* competitor; UI/UX reference |

(A fourth was skimmed because the first three are libraries rather than web tools, and a hosted
app was needed for the UX-pattern half of the scan.)

## Table stakes observed

| # | Capability | Seen in | Verdict | Where it landed |
|---|------------|---------|---------|-----------------|
| 1 | Cubic smoothing spline minimising `Σ wᵢ(yᵢ−g(xᵢ))² + λ∫g″²` | 1, 2, 3 | in-model | core algorithm (Reinsch / Green–Silverman banded solve) |
| 2 | `p ∈ [0,1]` smoothing parameter (0 = straight line, 1 = interpolate) | 2 | in-model | `mode=smoothing` + `smoothing` param (page slider) |
| 3 | Raw penalty `λ ≥ 0` | 1, 3 | in-model | `mode=lambda` + `lambda` param |
| 4 | Target effective degrees of freedom | 3 | in-model | `mode=df` + `df` param (bisection on log λ) |
| 5 | Automatic selection by generalized cross-validation | 1 (`lam=None`), 3 (`cv=FALSE`) | in-model | `mode=auto`, `criterion=gcv` (default) |
| 6 | Ordinary leave-one-out CV as an alternative criterion | 3 (`cv=TRUE`) | in-model | `criterion=cv` (exact LOO via the hat diagonal) |
| 7 | Per-observation weights | 1, 2, 3, 4 | in-model | `weights` param |
| 8 | Evaluate the fit at arbitrary new x | 1 (callable), 2 (`xx`) | in-model | `predict_at` param |
| 9 | Dense curve for plotting | 2, 4 | in-model | `resample` param (evenly spaced, N points) |
| 10 | Report effective df / λ / criterion score | 3 (`df`, `lambda`, `cv.crit`, `pen.crit`) | in-model | JSON: `effective_df`, `lambda`, `gcv`, `cv`, `penalized_criterion` |
| 11 | Report residuals + fit error (RMSE) | 3, 4 (RMSE hint) | in-model | JSON `points[].residual`, `rss`, `rmse` |
| 12 | Duplicate-x handling (average / bin) | 2 (averages), 3 (bins by `tol`) | in-model | exact-duplicate x merged by weighted mean, count reported |
| 13 | Piecewise-polynomial coefficient export | 2 (`ppform`), 4 (B-spline/NURBS export) | in-model | `coefficients=true` → `pieces[]` (breaks + cubic coefficients) |
| 14 | Plot of raw vs smoothed | 2, 4 | in-model | `output=svg` (self-contained SVG chart) |
| 15 | Minimum-data / strictly-increasing-x validation | 1, 3 (≥4 distinct x) | in-model | ≥4 distinct x enforced; unsorted input is sorted, not rejected |
| 16 | Smoothing-parameter **slider** UX | 2 (Curve Fitter app), 4 | in-model | `page/meta.toml` `kind = "slider"` on `smoothing` |
| 17 | Preset configurations | 2 (example gallery), 4 | in-model | `[[example]]` preset chips on the page |
| 18 | CSV / tabular round-trip of the result | 4 (file in, table out) | in-model | `output=csv` |

## Deliberately NOT built (out of model / out of scope)

Listed, not implemented — each is either outside this repo's pure-Rust/WASM tool shape or a
different tool entirely:

* **Spline degree `k ≠ 3`** (SciPy `k=1..5`). The classical smoothing spline is cubic; a general-`k`
  penalised B-spline needs a different solver (and a different roughness penalty). Cubic only.
* **Automatic knot selection / `nknots`, `all.knots`** (R, SciPy FITPACK `s`-based knot insertion).
  This tool uses every distinct x as a knot, which is the exact smoothing-spline formulation and
  is O(n) at our size cap. A separate knot-reduction tool would be a different backlog row.
* **`s`-parameterised FITPACK smoothing** (`splrep(s=…)`, "residual budget" formulation). Different
  criterion from the penalised one; supporting both would make the reported `lambda`/`df` ambiguous.
* **Multivariate / gridded / bivariate smoothing** (`csaps` cell-array input, `bisplrep`,
  `SmoothBivariateSpline`). 2-D surface fitting is a distinct tool.
* **Parametric curves in d > 1** (`splprep`, SplineCloud NURBS control-point editing).
* **Interactive control-point dragging and an interactive knot editor** (SplineCloud). This repo
  renders a declarative form + text/SVG output; there is no canvas-editing surface.
* **Spline integral / root-finding** (`splint`, `sproot`). The first derivative *is* reported
  (`slope` on every predicted/resampled point) because it falls straight out of the piecewise
  coefficients; integrals and roots are separate operations that belong to an evaluator tool.
* **R's `spar` value.** `spar` is defined through a basis-dependent trace ratio
  (`λ = r · 256^(3·spar−1)`), so a number reported here would not be reproducible in R. `lambda`,
  `smoothing` and `effective_df` are reported instead, and the FAQ says so.
* **Roughness weights `λ(t)` varying per interval** (`csaps` vector `p`). Single global penalty only.
* **Date/time x-axis parsing.** None of the three reference implementations accept dates; x must be
  numeric here too (use a day index or epoch seconds). Stated on the page and in the error message.
* **Image-of-a-graph digitising** (SplineCloud). Out of scope.
* **Cloud storage / shareable model IDs / REST API** (SplineCloud). This repo is offline-first;
  the page's `?param=` deep links are the sharing story.

## Design decisions that differ from the references (and why)

* **`smoothing` (p) is scale-invariant here.** MATLAB's `p` depends on the x units, which is why its
  docs need an `h³` default heuristic. This tool computes the penalty on x rescaled to `[0,1]`, so
  `λ_reported = λ_normalised · (xmax−xmin)³` and a given `p` means the same thing whether x is in
  seconds or days. `mode=lambda` still takes/returns λ in the input's own x units, matching SciPy.
* **`p` default is 0.99, not 0.5.** The useful range of `p` bunches near 1 (the same reason MATLAB's
  default heuristic sits near 1); 0.5 is already almost a straight line for most data. The page
  slider therefore uses a 0.001 step and the preset chips cover the interesting decades.
* **Auto is the default mode**, matching SciPy `lam=None` and R `cv=FALSE`, because a user pasting a
  noisy series usually has no prior λ.
* **GCV/CV are guarded against the interpolation limit**: when `effective_df → n` both scores are
  reported as unavailable rather than a 0/0 artefact, so `auto` cannot select interpolation.

## Verification hooks derived from this scan

* One real run per `mode` value (`auto`, `smoothing`, `lambda`, `df`) and per `criterion`
  (`gcv`, `cv`) and per `output` (`json`, `csv`, `svg`).
* Non-default checkbox state: `coefficients=true`.
* Cap boundary: `MAX_POINTS` at and one over; `resample` at and one over its cap.
* Both accepted data shapes: y-only list and `x,y` rows (plus a JSON array), and a header row.
* `p = 1` must reproduce the data exactly (interpolation) and `p = 0` must be a straight line —
  the two endpoints the `csaps` docs define.
