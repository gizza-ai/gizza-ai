# Competitor analysis: anova

Date: 2026-08-29
Tool: `anova` — one-way ANOVA calculator for grouped observations and summary statistics.

## Scan summary

Web search query: `one-way ANOVA calculator Tukey HSD Welch Levene online`.

Reviewed the feature shape of the top visible one-way ANOVA calculators, without copying wording, branding, examples, or visual treatment. The scan included LearnBin's advanced one-way ANOVA calculator, StatsKingdom's one-way ANOVA + Tukey calculator, StatsUnlock's one-way ANOVA calculator, Pearson's ANOVA/F-critical calculator, and vol.io's one-way ANOVA calculator. The recurring competitor set accepts raw groups in separate columns, exposes common post-hoc choices, reports p-values/effect sizes, and often adds assumption checks or charts. The goal for this repository is a local, deterministic, browser/CLI/chat-compatible Rust implementation rather than a full statistical workstation.

## Table-stakes found

| Capability / UX pattern | In-model decision | Implementation notes |
| --- | --- | --- |
| Accept raw observations with one group per column | Built | `format=wide`, ragged columns allowed; default example uses a textbook wide table. |
| Accept long `group,value` rows | Built | `format=long`, auto-detection for labelled two-column data, header handling. |
| Accept summary statistics (`name,n,mean,sd`) | Built | `format=summary`; notes explain unavailable raw-data diagnostics. |
| Auto-detect delimiter and header | Built | `delimiter=auto` and `header=auto`, with explicit overrides for ambiguous data. |
| ANOVA table with SS, df, MS, F, and p-value | Built | Text, markdown table, and JSON outputs. |
| Group means and standard deviations | Built | Per-group n, mean, sd, sem, plus min/max/sum for raw input. |
| Significance level control | Built | `alpha` number with 0.0001-0.5 bounds and slider page control. |
| Critical F value | Built | Included in summary and JSON. |
| Effect size | Built | Eta-squared and omega-squared reported. |
| Unequal-variance alternative | Built | Welch's ANOVA reported when each group has non-zero sample variance. |
| Variance homogeneity check | Built | Brown-Forsythe Levene test for raw observations. |
| Pairwise post-hoc comparisons | Built | Tukey/Tukey-Kramer, Fisher LSD, Bonferroni, and Holm modes. |
| Export / copy-friendly output | Built | Readable summary, markdown tables, and structured JSON. |
| Example datasets / preset chips | Built | Three page examples: wide data, long rows + Tukey, summary stats + JSON. |
| Clear limits and assumptions | Built | Page lists max values/groups and model assumptions. |

## Out-of-model or deliberately rejected

| Feature | Reason |
| --- | --- |
| Two-way ANOVA, repeated-measures ANOVA, MANOVA, ANCOVA, mixed models | Different statistical designs with additional factor/subject/covariate models; would need separate tools to avoid schema bloat and misleading results. |
| Spreadsheet upload / XLSX parsing | This tool is text-first and browser-local; pasted CSV/TSV covers the common no-upload workflow. A future file-to-table tool can feed it. |
| Residual plots and diagnostic charts | Plotting residuals would be in-model but materially expands the UI and output contract. Rejected for this first tool to keep the core statistic transparent; JSON output exposes enough values for external plotting. |
| Backend dataset storage, accounts, collaboration, or report hosting | Outside the local no-account model of this public toolkit. |
| Exact replication of every package's Tukey numerical algorithm | The implementation uses tested pure-Rust numerical approximations and known table checks. It targets practical calculator accuracy without depending on non-wasm native statistics libraries. |

## Resulting schema decisions

- `data` is the only required field and is multiline on the page.
- Fixed choices are enums: `format`, `delimiter`, `header`, `posthoc`, and `output`.
- Numeric controls are bounded: `alpha` (`0.0001..=0.5`) and `decimals` (`0..=10`).
- The default output is a readable report; markdown and JSON are explicit alternatives.
- The page copy states assumptions, limits, and when to use Welch/post-hoc output.
