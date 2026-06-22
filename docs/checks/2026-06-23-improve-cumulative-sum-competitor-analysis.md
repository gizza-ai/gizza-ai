# cumulative-sum — competitor analysis (2026-06-23)

Tool: **cumulative-sum** — running total (prefix sum) of a list of numbers, with
optional running average, minimum, and maximum. Surfaces: chat skill, CLI, page.

## Surface verification (Phase 1)

- **chat**: drift-guard schema test passes (`schema_json_matches_authored_chat_schema`);
  `wafer build` validates `target/block.wasm` OK (313.3 KiB). manifest.json matches the
  descriptor schema.
- **CLI**: `gizza tool cumulative-sum numbers="1,2,3,4"` → running-total table;
  `… average=true min=true max=true` adds the three extra columns; a non-numeric value
  errors naming the bad token.
- **page**: 4 Playwright tests pass (default running total, all three checkboxes,
  non-numeric error, query-param deep-link `?numbers=…&average=true`).
- 11 core unit tests pass.

## Competitors surveyed (top 5)

1. **onlinetools.com — Find the Running Total** (`/integer/find-running-total`)
2. **3roam.com — Running Total Calculator** (`/running-total-calculator/`)
3. **summe.org — Cumulative Sum Calculator** (`/cumulative_sum`)
4. **easycalculation.com — Cumulative Sum (CUSUM) Calculator** (statistics/cusum)
5. **excel-easy.com / extendoffice.com — Running Total in Excel** (how-to guides, not tools)

## Feature diff

| Capability | gizza cumulative-sum | onlinetools | 3roam | summe | easycalc CUSUM |
| --- | --- | --- | --- | --- | --- |
| Running total / prefix sum | yes | yes | yes | yes | yes (control-chart variant) |
| Comma / space / newline input (any mix) | yes | partial (comma or smart) | comma only | yes | comma |
| Decimals + negatives | yes | yes | yes | yes | yes |
| Running average column | yes | no | yes (single final avg) | no | no |
| Running minimum column | yes | no | no | no | no |
| Running maximum column | yes | no | no | no | no |
| Per-row table output (step breakdown) | yes | optional ("show summands") | no | yes | yes |
| Sum-index / row numbering | no (row order is implicit) | optional | no | no | yes |
| Runs locally / private / no sign-up | yes | yes | yes | yes | yes |

## Gap ranking (fit-to-model)

1. **Running min / max columns** — gizza already EXCEEDS every competitor here (none
   offer running min/max). No action needed.
2. **Running average** — matched (3roam offers only a single final average; gizza shows
   it per row). No action needed.
3. **Sum-index / row numbering** (onlinetools, easycalc) — cosmetic. Our table already
   presents one row per value in order, so the step is implicit and the per-row layout
   conveys the same breakdown the "show summands" option gives. Adding an explicit index
   column would be marginal and clutter the default output; deferred as not worth a schema
   change.

## Out-of-model / not built

- **CUSUM control-chart statistics** (upper/lower control limit, ARL — easycalculation):
  this is a different statistical tool (quality-control charting), not a running total.
  Out of scope for a plain cumulative-sum utility; would be its own tool.

## Conclusion

cumulative-sum meets or beats all five competitors on the core feature set and adds
running min/max that none of them provide. No in-model capability, copy, or UX gap
warranted a change. All three surfaces verified green; drift-guard regenerated (the
authored schema in `src/lib.rs` matches the descriptor).
