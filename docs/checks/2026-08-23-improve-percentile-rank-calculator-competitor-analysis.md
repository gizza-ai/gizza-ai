# percentile-rank-calculator — competitor analysis (2026-08-23)

Scan run before implementation. Findings below are paraphrased feature observations from reachable
public calculators; no competitor wording, branding, examples, or assets were copied into the tool.

## Competitors reviewed

| # | Tool | Reachable | What it offers |
|---|------|-----------|----------------|
| 1 | Omni Calculator percentile rank calculator | yes | One dataset field, one target value, a clear less-than-or-equal interpretation, a small dataset cap, and explanatory copy about ranking a score within a set. |
| 2 | StatsUnlock percentile rank calculator | yes | Dataset entry with comma/line-break separators, percentile rank, cumulative percentage, z-score, quartile position, and descriptive-statistics style output. |
| 3 | AcademyCalc percentile calculator | yes | Calculator that covers both percentile values and percentile ranks, step-oriented explanation, data-set input, target score input, and educational copy. |
| 4 | Bytevancer percentile calculator | yes | Percentile-value mode plus reverse percentile-rank mode, sorted-data display, Excel-compatible percentile-value notes, and worked examples. |
| 5 | Best Calculators percentile rank calculator | yes | Explicit NIST midpoint / inclusive method wording, below/equal/above counts, paste-a-class-set workflow, and educational examples. |

## Table stakes → in-model / out-of-model

| Table stake | Seen at | Verdict | Where it landed |
|---|---|---|---|
| Paste a reference dataset with flexible separators | all | in-model | `data` string parses commas, semicolons, whitespace, tabs, and newlines; page placeholder shows comma-separated data. |
| Rank a target value inside that dataset | all | in-model | `values` param; output reports percentile rank plus counts. |
| Multiple target values per run | some statistical tools, absent on simple calculators | in-model | `values` accepts up to 100 targets so repeated comparisons do not require rerunning the same dataset. |
| Tie-handling convention | Best Calculators, SciPy-style references | in-model | `method` enum: `weak`, `strict`, `mean`, `rank`; docs explain each. |
| Below/equal/above counts | Best Calculators and stats-focused tools | in-model | Each output row includes counts, making the percentile formula auditable. |
| Quartile / z-score / summary stats | StatsUnlock and descriptive-statistics tools | in-model | Optional dataset summary plus quartile and z-score per ranked value. |
| Rounding control | common numeric-tool pattern | in-model | `decimals` 0–6 slider. |
| Charts / histograms / visual CDF | some broader statistics sites | out-of-model for this text-first pure block | No SVG/chart output; the current generic page can display text robustly, and neighboring chart tools cover visualization. |
| Import CSV files | spreadsheet/statistics products | out-of-model | This block is a paste-field calculator; file parsing belongs in CSV-specific tools. |
| Percentile value ("find the 90th percentile") | AcademyCalc, Bytevancer, calculator.io | out-of-scope for this backlog row | Page explains the distinction. This tool does percentile rank (value → percentage), not percentile value (percentage → value). |

## Decisions this scan drove

1. **Expose tie handling as a first-class enum.** Competitors often hide the convention, which is why
   two calculators can disagree on tied values. The descriptor and page make `weak`, `strict`,
   `mean`, and `rank` explicit.
2. **Include counts in every result row.** Below/equal/above counts let users verify the formula by
   inspection and mirror the best educational calculators.
3. **Support multiple target values.** Ranking several scores against the same dataset is a common
   workflow and is cheap in this pure model.
4. **Keep percentile-value calculation out.** Several competitors combine percentile value and rank,
   but this backlog row is rank-specific; merging both would blur the descriptor and page copy.
5. **Add optional summary stats, not charts.** Text summary stats are model-fit and testable in CLI
   and browser surfaces; charts would add visual complexity without changing the rank result.

## Not copied

No competitor copy, page structure, tag line, worked example, or trademarked phrasing was reused.
