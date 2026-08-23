# random-projection-reducer — competitor analysis (2026-08-23)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased observations of publicly documented behaviour — no competitor
copy, branding or trademarks were reused.

## Tools reviewed (top 3 reachable)

| # | Tool | Reachable | Shape |
|---|------|-----------|-------|
| 1 | scikit-learn `sklearn.random_projection` (`GaussianRandomProjection`, `SparseRandomProjection`, `johnson_lindenstrauss_min_dim`) | yes | The reference implementation: library API, `n_components='auto'` driven by the JL bound |
| 2 | Weka `weka.filters.unsupervised.attribute.RandomProjection` | yes | GUI/CLI data-mining filter: fixed target attribute count or a percentage, three matrix distributions |
| 3 | Stack Abuse — "Random Projection: Theory and Implementation in Python with Scikit-Learn" | yes | The most-linked practitioner walkthrough; defines what users expect to *see* (distance-distortion diagnostics) |

No purely browser-based random-projection tool exists — the technique currently lives in
libraries and desktop data-mining suites, so #3 stands in for the "what does a user want on
screen" dimension that a hosted UI would otherwise supply.

## Table stakes observed → our decision

| Capability | Seen on | Fit | Where it landed |
|---|---|---|---|
| Project an `n × d` matrix onto `k` random directions, scaled so expected distances are preserved | 1, 2, 3 | in-model | core: `project()`; every distribution is normalised so `E‖Rx‖² = ‖x‖²` |
| **Gaussian** random matrix, entries `N(0, 1/k)` | 1, 2 | in-model | `method = "gaussian"` (default) |
| **Sparse / database-friendly** matrix `±√(s/k)` with probability `1/2s`, zero otherwise | 1, 2 | in-model | `method = "sparse"`, density auto = `1/√d` (scikit-learn's rule) |
| **Achlioptas** variant — density `1/3`, i.e. `√3·{−1, 0, +1}` at `1/6, 2/3, 1/6` | 1 (`density=1/3`), 2 (`SPARSE1`) | in-model | `method = "achlioptas"` |
| **Rademacher / ±1** matrix at probability `½` each | 2 (`SPARSE2`) | in-model | `method = "rademacher"` |
| **Explicit density** override for the sparse family | 1 (`density`) | in-model | `density` param, `0` = auto for the chosen method |
| **`n_components='auto'`** from the Johnson–Lindenstrauss bound | 1 | in-model | `components = 0` (default) → `k = ⌈4·ln n / (ε²/2 − ε³/3)⌉` |
| **`eps` distortion tolerance**, default `0.1` | 1 | in-model | `eps` param, default `0.1`, range `0.01–0.99` |
| Report the JL minimum dimension for the data at hand | 1 (`johnson_lindenstrauss_min_dim`), 3 | in-model | report has a "Johnson–Lindenstrauss guidance" block incl. the `ε = 0.5 / 0.2 / 0.1 / 0.05` table |
| **Reproducibility via a seed** (`random_state`) | 1, 2 (`-R`) | in-model | `seed` param, default `42`; a portable SplitMix64/xoshiro256++ stream, so the same seed gives identical numbers natively, in the CLI and in the browser |
| Fixed **target dimension count** | 1, 2 (`-N`, default 10) | in-model | `components` param |
| Target dimension as a **percentage of the input width** | 2 (`-P`) | in-model | `components` accepts `25%` — a percentage of the input column count |
| **Distance-preservation diagnostics** (absolute/relative distance differences, mean distortion, histogram) | 3 | in-model | report has mean / median / max distortion, mean ratio, and the share of pairs inside ±ε |
| Expose the **projection matrix** for reuse on new rows | 1 (`components_`) | in-model | `format = "matrix"` emits the `k × d` matrix as CSV |
| Machine-readable output for downstream plotting | 1, 3 | in-model | `format = "csv"` (projected rows) and `format = "json"` (everything, incl. diagnostics) |
| Header row / column names in a pasted table | 2 (ARFF attribute names) | in-model | first non-numeric row is auto-detected as a header and echoed in the report |
| Sensible failure when the JL bound exceeds the input width | 1 (raises) | in-model | we clamp to the input width and say so in the report instead of erroring — a small pasted matrix still produces a usable answer |

## Out-of-model (listed, not built)

- **Sparse/CSR input and `dense_output=False`** (1) — our surfaces take a pasted dense table and
  return text; a sparse in-memory representation has no serialisation on this input model.
- **`inverse_transform` / `compute_inverse_components`** (1) — the pseudo-inverse of a `k × d`
  matrix is a second `d × k` dense matrix and a least-squares solve; it is a *lossy* reconstruction
  that only makes sense inside a fitted pipeline object, which a one-shot tool has no place to hold.
  `format = "matrix"` is the honest substitute: it hands back the exact matrix used.
- **`fit` on one dataset then `transform` another** (1, 2 batch filtering) — needs persistent state
  across two invocations. Mitigated: the seed plus `format = "matrix"` fully determines the
  projection, so the identical matrix is reproducible on demand.
- **`NominalToBinary` pre-conversion of categorical attributes** (2) — a separate encoding concern;
  the input contract here is a numeric matrix, and non-numeric cells are a named error.
- **Class-attribute preservation** (2) — an ARFF/model-training notion with no analogue in a
  standalone reducer; a label column would just be projected as a number, so it is rejected as input.
- **Plots** — scatter of the first two projected dimensions, distortion heat-map, ratio histogram
  (3). The page renders one text result; the `csv` format exists precisely so the numbers go
  straight into a plotting tool.

## Extras we ship that the scanned tools do not

- **Distortion diagnostics in the same call as the projection** — #1 computes the projection and
  leaves distance checking to the user; #3 shows how to write that check by hand. Ours reports it
  by default, including how many sampled pairs actually landed inside the requested ±ε.
- **A JL guidance table** for several ε values against the pasted row count, so the ε/`k` trade-off
  is visible without a second call.
- **Byte-identical results across chat, CLI and the browser page** — the RNG is a fixed integer
  stream, not a platform RNG, so a `seed` is portable in a way `random_state` (NumPy's Mersenne
  Twister) is not across languages.
- Runs entirely **client-side**: nothing is uploaded, unlike a hosted notebook.

## Known limits (stated on the page)

- Up to 2,000 rows, 1,000 columns and 200,000 cells; target dimension `k` between 1 and 256.
- Distortion diagnostics sample at most 20,000 row pairs (all pairs when there are ≤ 200 rows).
- Random projection is *not* variance-optimal — PCA (`principal-component-analysis`) finds better
  axes for the same `k`; random projection wins on speed and on data too wide for an
  eigen-decomposition, and its guarantee is about pairwise distances, not variance.
- The JL bound depends only on the row count, so tiny pasted tables ask for more dimensions than
  they have columns; the report says so rather than pretending the guarantee holds.
- All reported numbers are rounded to 6 decimal places.
