# csv-merge — competitor analysis (2026-06-20)

Thirty-second `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) tool;
`Input::None` + a `files` source_list (like merge-pdf). Surfaces: chat + CLI
(no page — array input). Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| "merge/combine CSV" tools | stack many CSVs into one, handle differing headers, in-browser | capabilities |
| join tools | key-join (VLOOKUP-style) on a shared column | capabilities |

## Gap diff vs our tool
Our tool: concatenate (stack) ≥2 CSVs. With headers, the output uses the UNION of
all headers (first-seen order) and aligns each file's rows by column name (missing
→ blank); without headers, rows stack positionally. Covers the core "combine many
CSVs into one" (UNION-style) case.

**In-model gaps considered, deferred (the row says "or key-joins"):**
- **Key-join** (inner/left join on a shared key column across files) — meaningfully
  more complex (join semantics, which file is left, duplicate keys). Documented as
  a deferred mode / sibling `csv-join` tool; v1 concatenates.
- **Source/filename column** — tag each row with which file it came from (pairs
  with csv-insert-column).

**Out-of-model:** none notable.

## Tested
unit (4: concat same header, header-union aligns differing columns with blanks,
no-header positional stacking, errors for <2 inputs) + drift-guard · `wafer build`
validates the block (csv → wasm; pure-Rust so also works in the chat SW) · CLI
merges two real public CSVs (header + stacked rows) + <2-files error path. No page.

> Original work only — no competitor copy, branding, or trademarks copied.
