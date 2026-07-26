# weighted-random-sampler competitor analysis (2026-07-26)

## Tool scope

`weighted-random-sampler` draws rows from a CSV table or JSON array with probability proportional to a numeric weight field. It supports reproducible seeded draws, sampling with or without replacement, and emits the result in the same data format as the input.

## Competitor / reference scan

Search queries used:

- `weighted random sample CSV JSON online tool weighted sampling with replacement`
- `weighted random picker online list weights CSV seed with replacement`

| Competitor / reference | Observed table-stakes | In-model decision |
| --- | --- | --- |
| qsv `sample` / data-wrangling samplers | CSV-row sampling commonly exposes sample size, seed/reproducibility, and multiple sampling modes. | Implemented CSV input, sample size, seed, delimiter/header handling, with/without replacement. Broader reservoir/bernoulli methods are out of scope for a focused weighted sampler. |
| CalcBe weighted random picker | Weighted draw tools expose item weights/ratios, optional seed for reproducible draws, and a clear winners result. | Implemented numeric weights, deterministic seed, and explicit sampled rows. Shareable URL is handled by the generic page deep-link pattern. |
| simplified.tools / online random pickers | Picker tools commonly support list/row input, weights, no-duplicate mode, repeated draws, seeded replay, and a draw result manifest. | Implemented no-duplicate mode as `replacement=false`, repeated draws as `replacement=true`, and seeded replay. Group caps/odds estimates/manifests are out of model for this first block. |
| Konvi-style picker utilities | Common UX includes weighted selection, batch count, no-duplicate mode, seeded determinism, and CSV/JSON export. | Implemented `n`, replacement toggle, deterministic seed, CSV and JSON output. Animated reveal/export UI is not part of gizza’s tool model. |

## Table-stakes matrix

| Capability | Status | Notes |
| --- | --- | --- |
| CSV row input | In-model, implemented | Header row can be preserved and named weight columns are supported. |
| JSON array input | In-model, implemented | Weight field is an object key; numeric strings are accepted. |
| Weight column/key | In-model, implemented | Weights must be non-negative finite numbers. |
| Sample size | In-model, implemented | `n` controls rows/draws. |
| With replacement | In-model, implemented | Allows repeated rows and `n` larger than source row count. |
| Without replacement | In-model, implemented | Uses weighted no-duplicate selection and preserves original row order. |
| Seeded deterministic output | In-model, implemented | A splitmix64 PRNG keeps CLI/page/chat results stable for the same seed. |
| CSV delimiter/header controls | In-model, implemented | Comma, tab, semicolon, and pipe delimiter choices. |
| Output format matching input | In-model, implemented | CSV in -> CSV out; JSON array in -> JSON array out. |
| Animated wheel / reveal UI | Out of model | Visual game-style selection is not needed for the block surface. |
| Group quotas/caps or stratification | Deferred/out of scope | Useful but materially more complex than row-level weighted sampling. |
| Inclusion probability estimates | Deferred/out of scope | Could be a separate statistical report mode; not necessary for a row sampler. |
| Cryptographic randomness | Out of scope | Reproducibility is a stronger requirement for this tool; seed controls output. |

## UX/control decisions

- `format` and `delimiter` are enums for fixed choices.
- `replacement`, `header`, `n`, and `seed` use simple controls and deterministic defaults.
- `data` is a multiline text input so CSV and JSON can be pasted directly.
- Examples cover CSV without replacement, CSV with replacement, and JSON-array sampling.

## Verification plan

- Core tests cover deterministic seeded CSV and JSON sampling, with-replacement draws, invalid weights, missing weight fields, zero weights, and unsupported formats.
- CLI verification should include an exact seeded output case.
- Page verification should assert the same exact output and a deep-link/preset case.
- `web/pkg` is a local generator cache and must not be committed.
