# target-mean-encoder — competitor analysis (2026-07-26)

Scan done BEFORE implementation. All findings paraphrased — no competitor copy, branding,
or trademarks reproduced.

## What the tool does

Target mean encoding (a.k.a. mean / impact / likelihood encoding): replace a categorical
column with the average of a numeric target per category, so a model can consume the
category as a single numeric feature. Optional smoothing shrinks rare-category means toward
the overall (prior) mean to fight overfitting.

## Competitors scanned

| # | Tool | Type | URL |
|---|------|------|-----|
| 1 | category_encoders `TargetEncoder` | library (Python) | contrib.scikit-learn.org/category_encoders/targetencoder.html |
| 2 | feature-engine `MeanEncoder` | library (Python) | feature-engine.trainindata.com |
| 3 | scikit-learn `TargetEncoder` | library (Python) | scikit-learn.org (preprocessing) |
| 4 | H2O Target Encoding | library/platform | docs.h2o.ai target-encoding |
| 5 | Various tutorials (Towards Data Science, ML-Digest, apxml) | educational | (worked-example patterns) |

These are the real, authoritative implementations people copy the semantics of; there is no
mainstream browser-based single-CSV "paste and encode" competitor, so the gizza page is the
differentiated surface (private, no upload, instant) over the same math.

## Table-stakes params / defaults / formulas (paraphrased)

- **Smoothing weight.** feature-engine uses the additive/m-estimate blend
  `w = n / (n + s)`, `encoding = w·(category mean) + (1 − w)·(global mean)`, with `s`
  user-chosen (default: no smoothing → raw mean). category_encoders defaults `smoothing = 10`
  with a companion `min_samples_leaf = 20` driving an S-curve weight. sklearn defaults
  `smooth = "auto"` (empirical-Bayes estimate). Consensus table-stake: an explicit numeric
  smoothing strength that pulls small categories toward the prior; `0` = raw mean.
- **Prior / global mean fallback.** All three fall back to the overall target mean for
  unseen/blank categories (`handle_unknown="value"` in category_encoders). Alternatives:
  return NaN, or raise an error.
- **Leakage control.** The headline concern of every tutorial. Real libraries prevent it via
  cross-fitting (out-of-fold means) or leave-one-out means (category_encoders
  `LeaveOneOutEncoder`): each row's encoding excludes its own target so the feature can't
  memorize the label.
- **Output shape.** Replace the column in place (default in most libraries) vs. keep the
  original and add a new encoded column.
- **Target type.** Binary (0/1) or continuous regression targets — the mean works for both.
  Multiclass is out of scope for a single-column mean.
- **Worked example.** Every tutorial shows a small table (e.g. City → churn) with per-category
  means and the smoothed result — the page must ship one.

## In-model decisions (shipped in the descriptor)

| Capability | Decision |
|---|---|
| Pick categorical column + numeric target column (by name or 1-based index) | ✅ `category`, `target` |
| m-estimate smoothing `w = n/(n+m)` toward the global prior; `0` = raw mean | ✅ `smoothing` (number, default 0) |
| Leave-one-out means (leakage control, deterministic) | ✅ `leave_one_out` (bool, default false) |
| Replace column in place vs append `<col>_target_enc` | ✅ `output` enum replace/append |
| Blank / all-missing category fallback: global mean / NaN / zero | ✅ `unknown` enum |
| Round encoded values | ✅ `decimals` (default 6) |
| Header row + delimiter handling (comma/tab/;/\|) | ✅ `header`, `delimiter` |
| Binary AND continuous targets | ✅ (plain numeric mean covers both) |

## Out-of-model / considered, not built

- **Cross-fold (K-fold) out-of-fold encoding.** Needs a random fold assignment (seeded RNG)
  and is non-deterministic across runs; leave-one-out gives the same leakage protection
  deterministically in a single pass, so LOO is shipped instead. Listed, not built.
- **sklearn `smooth="auto"` empirical-Bayes.** Per-category variance estimate; the explicit
  numeric `smoothing` covers the same intent with a transparent, testable formula. Considered,
  not built (would add opaque math without a clear browser UX win).
- **Simultaneous multi-column encoding / a fitted transformer object reused on new data.**
  Needs a persisted train→test split model; gizza tools are single-CSV, stateless, browser-local.
  Out of model. (Run the tool once per column.)
- **Multiclass target expansion (one encoded column per class).** Out of scope for a single
  mean column; listed.

## UX controls

- Preset example chips (raw mean, smoothed, leave-one-out, append-column) — competitors'
  tutorials all lead with worked examples, so the page ships `[[example]]` chips.
- `output`/`unknown` render as `<select>` (enumv); `leave_one_out` as a checkbox;
  `smoothing`/`decimals` as number fields; `data` as a multiline textarea.
