# outlier-detector — competitor analysis & improvement snapshot (2026-06-22)

## Tool

`blocks/outlier-detector` — flags outliers in a list of numbers using three standard
methods: **z-score**, **modified z-score (median + MAD)**, and **IQR (Tukey's fences)**.
Pure-Rust → runs on all backends (chat / CLI / in-browser page). Output: per-method
flagged values **with their list index** plus the supporting statistics (mean, sample
std dev, median, MAD, Q1/Q3/IQR, fences).

Params: `numbers` (required string), `z_threshold` (number, default 3 — also the cutoff
for the modified z-score), `iqr_k` (number, default 1.5).

## Surfaces verified (Phase 1)

- **Chat / LLM schema:** drift-guard unit test `schema_json_matches_authored_chat_schema`
  passes; `wafer build` validates the block.wasm (332.4 KiB, instantiates).
- **CLI:** `gizza tool outlier-detector numbers="..." [z_threshold=..] [iqr_k=..]` returns
  structured JSON for all three methods. Error path (`numbers="1 2 abc"`) reports a clean
  message.
- **Page:** `/tools/outlier-detector/` — Playwright `tool-page-outlier-detector.spec.ts`
  (2 tests) green: default run flags the outlier under z-score/modified-z/IQR sections;
  custom `z_threshold=2` re-flags via the z-score method; number fields pre-fill from the
  schema defaults (3 and 1.5).
- **Unit tests:** 12 core tests (mixed separators, empty/non-numeric rejection, constant
  data → no div-by-zero, IQR fence vectors, z-score & modified-z flagging, custom
  thresholds).

## Top competitors surveyed

1. **8gwifi — Outlier Detection Calculator** (8gwifi.org) — IQR, z-score, **modified
   z-score (MAD)**; consensus across methods; scatter plot; Python export.
2. **GetZenQuery — Outlier Calculator** — IQR with customizable fence multiplier,
   z-score with adjustable threshold, Tukey's fences.
3. **StatSolve Pro — Outlier Detection Calculator** — IQR fence method + z-score, with
   step-by-step working shown.
4. **AiMathCalculator — Outlier Calculator** — IQR fences, classical z-scores, **robust
   modified z-scores**; method picker with guidance on when to use each.
5. **Calculator Academy / miniwebtool — Outlier Calculator** — IQR method (Q1, Q3, fences),
   paste comma/space-separated data, flags below lower / above upper fence.

## Gap diff & ranking (fit-to-model)

| Capability | Competitors | gizza before | Action |
|---|---|---|---|
| IQR / Tukey's fences with adjustable k | all | yes (`iqr_k`) | kept |
| Z-score with adjustable threshold | most | yes (`z_threshold`) | kept |
| **Modified z-score (median + MAD)** | 8gwifi, AiMathCalculator | **missing** | **ADDED** (robust method; same threshold) |
| Report the flagged value's **position/index** | varies | yes | kept (differentiator — most show only the values) |
| Show supporting stats (mean/std, Q1/Q3/fences) | most | yes; added median + MAD | extended |
| Multiple separators (space/comma/semicolon/newline) | most | yes | kept |
| Privacy: 100% in-browser, no upload | rare | yes (wasm) | kept (differentiator) |
| Headless CLI + chat/LLM API surface | none | yes | kept (differentiator) |

### Closed this pass

- **Modified z-score (MAD) method** — the one genuine capability gap vs. the strongest
  competitors (8gwifi, AiMathCalculator). It is the robust complement to the classical
  z-score: it uses the median and MAD, so a few extreme points can't inflate the spread and
  mask each other (verified: for `1 2 3 4 5 6 7 8 9 200`, classical z-score at threshold 3.5
  flags nothing because std dev is dragged to 61.7, while modified-z and IQR both flag 200).
  Added to core, descriptor/manifest schema (no new param — reuses `z_threshold`), web
  summary, page copy, and tests; drift-guard regenerated.

### Out of model / deliberately not built

- **Scatter / box plots and Python code export** (8gwifi) — visualization/codegen are out of
  the pure-compute, single-output tool model.
- **"Consensus across methods" highlight** — the structured per-method output already lets a
  caller intersect the flagged indices; a separate consensus field would be redundant.

## Copy / branding

No competitor copy, branding, or trademarks were used. All titles, hero text, and content
are original.
