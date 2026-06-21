# csv-transpose — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/csv-transpose` — transpose a CSV (swap rows and columns).
Pure-Rust (`csv`). Pure-text input → text output: chat + CLI + a page. Adds the
row/column-swap operation to the csv-* family.

## What competitors do

- **Spreadsheets** ("Paste Special → Transpose") — the usual way, but manual and
  in-app.
- **`csvtool transpose` / pandas `df.T` / awk** — local + correct, but need a
  CLI/Python toolchain.
- **Online CSV transposers** — easy, but the data is uploaded.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The CSV never leaves the device.
2. **Correct, rectangular output.** Real CSV parsing (quoted fields handled), and
   **ragged rows are padded** with empty cells so the transpose is well-formed —
   something naive split-on-comma transposers get wrong.
3. **Round-trippable.** Transposing twice returns the original (verified by test).
4. **Delimiter-flexible** (`,` / tab / `;` / `|`).
5. **Agent-friendly.** One call to flip a wide table to tall (or vice versa) from
   chat, CLI, or a `?data=…` page.

## Honest scope

- A pure structural transpose — it doesn't re-key, aggregate, or pivot by value
  (see csv-pivot / csv-group-by for those).

## Tests

7 core unit tests: square and rectangular transpose; header row becomes the first
column; **ragged rows padded**; **double-transpose round-trips** to the original;
tab delimiter; and errors (empty input, bad delimiter). Plus the block drift-guard
schema test. **CLI verified** end-to-end. **Page** verified with Playwright. `wafer
build` instantiates the chat block (340 KiB).
