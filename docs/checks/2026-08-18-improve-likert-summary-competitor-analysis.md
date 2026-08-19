# likert-summary — competitor analysis (2026-08-18)

Scan run **before** implementing, per `create-next-tool` step 4. One WebSearch
("Likert scale survey analysis tool per-item mean top-2-box stacked bar chart online
calculator") plus a skim of the top 3 reachable competitor tools. All notes are
**paraphrased observations of capability** — no competitor copy, branding, or
trademarks are reproduced or used anywhere in this tool.

## Competitors skimmed

1. **StatsCalculators — Likert scale analysis calculator** (statscalculators.com).
   Paste/upload data or load a sample; pick which columns are Likert items; scale
   range auto-detected or user-set; missing values dropped automatically. Outputs
   per-item mean/median/mode/SD, response-distribution percentages, item ranking by
   mean, floor/ceiling-effect flags, stacked bar charts, and a formatted report block.
2. **Fomr — Likert scale chart maker** (fomr.io). Frequency-count entry (how many
   respondents chose each option) plus an optional question label; three scale presets
   (5-point agreement, 5-point satisfaction, 7-point agreement); outputs top-box %,
   bottom-box %, mean, and a **diverging** stacked bar (negatives one side, neutral
   centred, positives the other) with a PNG export. Runs client-side.
3. **BrainMatters — Likert scale calculator** (brainmatterslearning.com). Two input
   modes (raw respondent×item rows, or frequency tallies); 4-point / 5-point / custom
   min–max scales; label sets (agreement, frequency, generic, or custom comma-separated
   labels); **reverse scoring** of named item columns via `x' = min + max − x`; missing
   handling (exclude, complete-cases/listwise, or prorate by respondent mean); custom
   missing markers; adjustable rounding 0–6 decimals. Outputs valid n + missing counts,
   weighted mean, overall scale mean, median, mode, distribution table, per-item means,
   and an equal-interval interpretation table.

## Table-stakes → where each one landed

Every table-stake below ends in the descriptor **or** the out-of-model list — none dropped.

| Table-stake (seen at 1+ competitors) | Fit | Where it landed |
| --- | --- | --- |
| Raw respondent × item data paste | in-model | `data` (required, multiline textarea) |
| Frequency-count entry mode | in-model | `input = counts` (rows = item, cells = tally per scale point) |
| Scale size 4 / 5 / 7 / custom points | in-model | `points` (2–11, default 5) |
| Named label sets (agreement / satisfaction / frequency / quality) | in-model | `scale` enum + `[[example]]` preset chips |
| Custom comma-separated labels | in-model | `scale = custom` + `labels` |
| Answers given as label text, not just numbers | in-model | core matches labels case-insensitively (exact, then unique prefix) |
| Reverse-scored items (`x' = min + max − x`) | in-model | `reverse` (comma list of item names or 1-based indices) |
| Per-item mean | in-model | always in the per-item table |
| Median, mode, SD | in-model | always in the per-item table |
| Response distribution counts + % | in-model | per-item distribution rows |
| Top-box / top-2-box + bottom-box % | in-model | `box_size` (1..points/2, default 2) → Top/Bottom columns |
| Neutral / midpoint reporting | in-model | Neutral % column for odd `points` |
| Missing / blank handling | in-model | `missing = exclude` (pairwise) or `listwise` (complete cases only) |
| User-defined missing markers | in-model | blanks plus the usual `NA`/`N/A`/`-`/`.` markers are treated as missing |
| Item ranking by score | in-model | `sort = input / mean-desc / mean-asc / top-desc` |
| Stacked bar visualisation | in-model | text stacked bars (`chart`, on by default) |
| Diverging (neutral-centred) stacked bars | in-model | `diverging` checkbox |
| Adjustable rounding | in-model | `decimals` (0–6, default 2) |
| Valid n + missing counts | in-model | per-item `n` and `missing` columns |
| Overall scale mean across items | in-model | summary line under the table |
| Floor / ceiling effect flags | in-model | flagged when an end category takes ≥ a majority share |
| Non-comma delimiters (TSV exports) | in-model | `delimiter` |
| Scale-reliability (Cronbach's α) | in-model | `alpha` checkbox (needs ≥2 items, listwise-complete rows) |

## Out-of-model (listed, deliberately not built)

- **PNG / image chart export.** The page renders text output; the bars are text so they
  copy into any doc. A rasterised chart download is a different output model (see the
  repo's dedicated chart blocks for SVG/image charts).
- **Colour styling of bars** (warm/cool/gray palettes). Text output has no palette.
- **File upload / spreadsheet import.** Pure blocks take pasted text; upload is a
  media-input model.
- **Prorate-by-respondent-mean missing handling.** Imputation is a modelling choice that
  silently changes the data; `exclude` and `listwise` cover the defensible cases.
- **APA-style prose report generation.** Free-text narrative is an LLM job, not a
  deterministic pure block — the chat surface can write it from this tool's numbers.
- **Chi-square / association testing between questions.** Already covered by the
  existing `survey-tabulator` block; not duplicated here.
- **Saved datasets / accounts / dataset export.** No server, no state in this repo.

## Dup check

`ls blocks/ | grep -iE 'likert|survey|stat|summar|distrib'` surfaced `survey-tabulator`,
`descriptive-stats`, `csv-stats`, `frequency-distribution`. Read
`blocks/survey-tabulator/core/src/lib.rs`: it does **nominal** frequency tables and
two-way crosstabs with chi-square — no ordinal coding, no per-item means, no
top/bottom-box, no reverse scoring, no stacked bars. `descriptive-stats`/`csv-stats`
summarise continuous numeric columns, not ordered scale categories with labels. Distinct
model — built, not skiplisted.

## UX control patterns adopted

- `[[example]]` preset chips replace the competitors' scale-preset buttons (5-point
  agreement, 7-point agreement, satisfaction counts, diverging + reverse-scored).
- `multiline = true` on the data field so pasted rows keep their newlines.
- `[input.labels]` gives the `scale`, `missing`, and `sort` selects friendly labels while
  the canonical enum values stay stable for the CLI and chat schema.
- Checkbox controls for `chart`, `diverging`, and `alpha` (booleans) as competitors ship
  toggles rather than free text.
