# csv-to-yaml — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/csv-to-yaml` — convert a CSV table into a YAML list of objects
keyed by the header row. Pure-Rust (`csv` + `serde_yml`). Pure-text input → text
output: chat + CLI + a page. Rounds out the csv-* converters (csv-json-convert,
csv-to-xml, csv-to-table) with YAML output.

## What competitors do

- **Online "CSV to YAML" converters** — paste CSV, get YAML; convenient but data is
  uploaded and type handling varies.
- **`yq` + `csvkit` / pandas + PyYAML** — local + correct, but need a CLI/Python
  toolchain and a couple of steps.
- **Hand-writing YAML** — error-prone for anything beyond a few rows.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The CSV never leaves the device.
2. **Real YAML, ordered keys.** Emits a proper YAML sequence of mappings via
   `serde_yml`, with **column order preserved** — not string-concatenated YAML.
3. **Smart, safe type inference.** Numbers, booleans and empty→null are inferred,
   but **leading-zero codes (`007`) and signed/odd numerics stay strings** so IDs
   aren't corrupted. Turn inference off to keep everything a string.
4. **Header-aware + delimiter-flexible** (`,` / tab / `;` / `|`), with a real CSV
   parser for quoted fields.
5. **Agent-friendly.** One call to turn a spreadsheet into a YAML config/fixtures
   file, identical from chat, CLI, and a `?data=…` page.

## Honest scope

- **CSV → YAML list of flat objects** — not nested YAML, anchors/aliases, or the
  reverse (YAML → CSV; see json-yaml-converter for JSON↔YAML).

## Tests

6 core unit tests: a YAML list of objects with number inference; numbers/booleans/
null inferred; inference-off keeps strings quoted; leading-zero stays a string;
no-header uses `col1…` keys; and errors (empty input, bad delimiter). Plus the
block drift-guard schema test. **CLI verified** end-to-end. **Page** verified with
Playwright. `wafer build` instantiates the chat block (456 KiB).
