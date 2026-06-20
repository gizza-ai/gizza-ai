# csv-group-split — competitor analysis (2026-06-20)

Thirtieth `/create-next-tool` backlog pick. Pure-Rust (`csv` + `zip`) tool. Output
is a ZIP of per-group CSVs → chat + CLI (no page — binary/zip output has no page
mode). Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| "split CSV by column" tools | one file per distinct key value, download as zip, in-browser | capabilities |
| split-by-row-count tools | chunk into N-row files | capabilities |

## Gap diff vs our tool
Our tool: split a CSV into one file per distinct value of a key column (header
name or 1-based index), each output keeping the header; bundle them into a single
ZIP (deflate). Filenames are the key value, sanitized + collision-deduped. Covers
the core split-by-column-value → zip feature.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Split by row count / max size** (chunk every N rows) — a different split mode;
  a `chunk_rows` param or sibling tool.
- **Filename template** (e.g. `dept_{value}.csv`).
- **Drop the key column** from each output (it's constant per file).

**Out-of-model:** writing separate files to a folder (we return one zip — the
browser/CLI gets a single artifact, which is the model).

## Tested
unit core (4: splits by key w/ header, no-header by index, unsafe key value
sanitized to a_b.csv, errors for empty/unknown-column/no-header) + block (zip
roundtrip, drift-guard) · `wafer build` validates the block (csv + zip →
wasm32-wasip1; pure-Rust so also works in the chat SW) · CLI splits into a 3-entry
ZIP (Python zipfile confirms entries + content + integrity) + error path. No page
surface.

> Original work only — no competitor copy, branding, or trademarks copied.
