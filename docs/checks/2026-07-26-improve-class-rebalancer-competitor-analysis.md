# class-rebalancer — competitor analysis (2026-07-26)

Scan done before shipping the completed page/descriptor. Findings are paraphrased; no competitor copy, branding, or trademarks are reused.

## What the tool does

Class rebalancing adjusts an imbalanced labeled table by resampling whole rows: duplicate rows from smaller classes (random over-sampling), drop rows from larger classes (random under-sampling), or combine both toward a target minority-to-majority ratio. A fixed seed makes the random selection reproducible.

## Competitors scanned

| # | Tool / docs | Type | Notes |
|---|-------------|------|-------|
| 1 | imbalanced-learn random over/under-sampling docs | Python library | Canonical random resampling semantics and sampling-strategy ratios. |
| 2 | scikit-learn class imbalance / resampling examples | Python ecosystem docs | Emphasizes class-count reports and leakage-aware preprocessing. |
| 3 | Dataiku visual ML sampling options | Platform UI | Shows strategy selectors, class ratios, and reproducibility controls. |
| 4 | H2O / AutoML class balancing docs | Platform/library | Uses automatic class balancing, over/under sampling limits, and seed-like reproducibility. |
| 5 | Common ML tutorials on random over-sampling vs under-sampling | Educational | Worked examples use tiny minority/majority tables and count-before/after comparisons. |

## Table-stakes params / defaults / examples

- **Label column selector.** Tools require choosing the target/class column; CSV/browser UX should accept a header name and an index fallback. Defaulting to the last column matches common labeled-table layouts.
- **Strategy.** Random over-sampling duplicates minority rows; random under-sampling drops majority rows; combined approaches can do both. `auto` is useful as the simplest default and maps to over-sampling so no original rows are lost by default.
- **Target ratio / sampling strategy.** Competitors expose either desired class counts or a ratio. A minority:majority ratio in `(0, 1]` is compact for a single-page tool: `1` fully balances, `0.5` keeps the minority at half the majority.
- **Seed / reproducibility.** Random pickers need a seed so repeated preprocessing is deterministic.
- **Shuffle.** Many workflows shuffle after resampling; making it optional preserves readable input order by default.
- **Before/after counts.** Competitor docs highlight count diagnostics. A summary output mode should report per-class before/after counts and totals.
- **Worked example.** The standard example is a small labeled table with one minority row and several majority rows, showing duplicated minority rows or dropped majority rows.

## In-model decisions shipped

| Capability | Decision |
|---|---|
| Choose label column by header name, 1-based index, or blank last-column default | ✅ `label_column` |
| Random over-sampling | ✅ `strategy=oversample` / `auto` |
| Random under-sampling | ✅ `strategy=undersample` |
| Combined grow/drop to a common target size | ✅ `strategy=combine` |
| Ratio control | ✅ `target_ratio` numeric slider, `0.01..1.0` |
| Reproducible random selection | ✅ `seed` |
| Optional row shuffle | ✅ `shuffle` |
| Header handling | ✅ `header` |
| CSV output or count report | ✅ `output=csv|summary` |

## Out-of-model / considered, not built

- **SMOTE / synthetic samples.** Needs nearest-neighbor feature-space interpolation, numeric feature typing, and synthetic row generation. The current gizza model is a deterministic single-file browser tool; this tool intentionally resamples whole rows only.
- **Train/test split aware fitting.** A persisted fitted sampler and split management would be a multi-step ML workflow. This tool transforms one pasted CSV at a time.
- **Cost-sensitive model weights.** Class weights are model-training parameters, not a CSV transformation.
- **Per-class custom counts for many labels.** Powerful but too bulky for the generic tool form; the ratio handles common binary and multiclass cases.

## UX controls

- Preset chips: oversample-to-balance, undersample-summary, and combine+shuffle.
- `strategy` and `output` render as selects via enum schema and labels.
- `target_ratio` is a slider with numeric bounds matching the descriptor.
- `header` and `shuffle` are checkboxes.
- `data` is a multiline textarea; `label_column` and `seed` are text/number fields with placeholders.
