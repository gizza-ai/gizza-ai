# per-capita-normalizer — competitor scan (2026-08-16)

Scan run before implementation, per the create-next-tool recipe. Notes are paraphrased from public docs and calculator pages; no competitor copy, branding, or trademarks are reused.

Search: "per capita calculator per 100000 population rate calculator normalize counts by population".

## Competitor profiles

### 1. Public-health rate calculators
- **Pattern:** web forms that ask for a numerator count, denominator population, and a multiplier such as 1,000 or 100,000.
- **Table-stakes options:** numerator, denominator, rate base, decimal rounding, and a clear formula.
- **Outputs:** one rate and sometimes confidence intervals; most do not handle a multi-row pasted table.
- **Fit decision:** multi-row crude rates and bases are in-model; confidence intervals are listed as out-of-model for this first tool because they require distribution assumptions and a separate uncertainty UI.

### 2. Spreadsheet recipes and statistics references
- **Pattern:** show the formula `(count / population) * base`, with examples for rates per 1,000, per 10,000, and per 100,000.
- **Table-stakes options:** selectable base, readable labels, and export to a spreadsheet-friendly format.
- **Outputs:** normalized columns that can be sorted or charted.
- **Fit decision:** built as CSV/Markdown/JSON/text output with sorting and labels.

### 3. Data portals with population units
- **Pattern:** population often appears in actual people, thousands, or millions, while event counts stay raw.
- **Table-stakes options:** explicitly declare population units, avoid silent scale mistakes, and keep warnings near small numerators.
- **Outputs:** rate columns and notes about unreliable small counts.
- **Fit decision:** built population-unit scaling and a configurable small-count flag.

## Table-stakes checklist → decision

| Capability | Fit | Decision |
| --- | --- | --- |
| Count / population formula | in-model | **built** |
| Bases: per capita, per 1,000, per 10,000, per 100,000, per 1,000,000 | in-model | **built** via `per` enum |
| Custom rate base | in-model | **built** with `custom_per` |
| Multi-row pasted table | in-model | **built**, up to 10,000 rows |
| Header detection | in-model | **built**: auto / yes / no |
| Delimiter choices | in-model | **built**: auto, comma, tab, semicolon, pipe |
| Population unit scaling | in-model | **built**: ones / thousands / millions |
| Output rounding | in-model | **built**: 0–6 decimal places |
| Sort by rate or keep input order | in-model | **built** |
| Spreadsheet/report export | in-model | **built**: CSV, Markdown, JSON, text table |
| Overall pooled rate and index | in-model | **built** to compare each row with the combined average |
| Small numerator reliability flag | in-model | **built** with configurable threshold |
| Confidence intervals | out-of-model for this tool | Listed, not built; would need method selection and uncertainty copy |
| Age-standardized rates | out-of-model | Listed, not built; requires age-band inputs and a standard population |
| Choropleth/map output | out-of-model | Listed, not built; visualization belongs in a separate tool |

## Build decisions

- Use the last two fields as count and population so labels can contain a delimiter, e.g. `Springfield, IL,10,1000`.
- Default base is per 100,000 because it is common for public-health/event-rate reporting.
- Default small-count flag is 20 events, but users can set it to 0 to turn flagging off.
- Emit a plain text rate chart in the default output, while CSV/Markdown/JSON stay machine/report friendly.
