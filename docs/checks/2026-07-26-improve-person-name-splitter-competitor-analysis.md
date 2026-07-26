# person-name-splitter — competitor analysis (2026-07-26)

Scan done before final implementation. Findings are paraphrased; no competitor copy, branding, or trademarks are reused.

## What the tool does

Person-name splitting turns one full-name field into structured components such as title, first, middle, last, and suffix. The useful browser-tool version works on CSV columns, keeps the rest of the row intact, and documents ambiguity instead of pretending every human name is unambiguous.

## Competitors scanned

| # | Tool / docs | Type | Notes |
|---|-------------|------|-------|
| 1 | Python `nameparser` documentation | Library | Common component model: title, first, middle, last, suffix; handles particles and comma-form names. |
| 2 | HumanName / name parsing examples | Library/tutorial | Emphasizes configurable titles/suffixes and ambiguous cultural cases. |
| 3 | Spreadsheet/CRM full-name split tutorials | Workflow docs | Users expect CSV-style append/replace columns and examples like `First Last` and `Last, First`. |
| 4 | Data-cleaning tools with split-name actions | SaaS/UI | Common controls are name column selection, keep original vs replace, and previewable output columns. |
| 5 | Contact import cleanup guides | Educational | Highlights suffixes (`Jr`, `III`), titles (`Dr`), apostrophes, hyphens, and review of ambiguous rows. |

## Table-stakes params / defaults / examples

- **Input table and name column.** Users need to paste CSV and choose the full-name column by header name or index. Blank-first-column default is useful for one-column name lists.
- **Component columns.** Competitors usually expose title, first, middle, last, and suffix. These are shipped as the five output components.
- **Append vs replace.** Data-cleaning UIs generally let users preserve the original field or replace it with components. Append is the safer default.
- **Header and delimiter controls.** CSV imports need header/no-header mode and comma/tab/semicolon/pipe delimiters.
- **Whitespace cleanup.** Trimming cells is expected by default, with an opt-out for exact preservation.
- **Worked examples.** Must include titles/suffixes (`Dr. John Smith Jr.`), particles (`Ludwig van Beethoven`), comma form (`Smith, Jane Q`), hyphen/apostrophe names, and a mononym/ambiguous case.
- **Ambiguity reporting.** Name parsing is heuristic; a summary mode should count ambiguous rows for review.

## In-model decisions shipped

| Capability | Decision |
|---|---|
| CSV input and name column selector | ✅ `data`, `name_column` |
| Append components or replace source field | ✅ `output=append|replace` |
| JSON ambiguity/count report | ✅ `output=summary` |
| Title/first/middle/last/suffix components | ✅ output columns |
| Common titles and suffixes | ✅ deterministic token lists |
| Particles like van/von/de/del/di/la/mac/mc | ✅ particle-aware surname grouping |
| Comma form `Last, First Middle` | ✅ parsed when cell is CSV-quoted as needed |
| Header and delimiter options | ✅ `header`, `delimiter` |
| Whitespace cleanup | ✅ `trim` |

## Out-of-model / considered, not built

- **Global/culture-specific name databases.** Would require large data files and probabilistic inference; not appropriate for a small deterministic WASM tool.
- **AI-based name understanding.** Out of scope for the pure local gizza model and would make results non-deterministic.
- **Gender or ethnicity inference.** Not a CSV splitting task and carries privacy/fairness risks.
- **Multi-name household parsing.** Splitting `John and Jane Smith` into multiple people is a different entity-extraction task.

## UX controls

- Preset chips for append, replace, and summary review.
- Multiline CSV textarea, name-column text input, output and delimiter selects, and header/trim checkboxes.
- Page copy states that parsing is heuristic and ambiguous rows should be reviewed.
