# csv-change-delimiter — competitor analysis (2026-06-20)

Twenty-fifth `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text tool,
all 3 surfaces. Distinct from csv-json-convert (CSV↔JSON) — this re-delimits
CSV↔DSV. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| convertcsv / "change delimiter" tools | pick input + output separator, requote correctly, in-browser | capabilities |
| TSV/CSV converters | comma↔tab↔semicolon↔pipe; handle quoted fields | capabilities |

## Gap diff vs our tool
Our tool: parse with the `from` separator, re-serialize with `to` (single char or
comma/tab/semicolon/pipe word). The `csv` crate fixes quoting for the new
delimiter — fields containing it get quoted, fields that no longer need quoting
are unquoted (both verified). Handles embedded quotes/newlines per RFC 4180.
Covers the core requote-correctly conversion.

**In-model gaps considered, deferred (minor):**
- **Quote style** (minimal / always-quote / never) — a `quoting` param.
- **Line-ending choice** (LF vs CRLF) — `csv` writes LF; a `crlf` toggle could be
  added.
- **Skip-empty-lines / trim** preprocessing.

**Out-of-model:** encoding conversion (e.g. latin-1 → utf-8) — separate concern.

## Tested
unit (5: comma→tab, tab→semicolon, requotes a field containing the new delim,
unquotes when no longer needed, empty/missing/invalid-delimiter errors) +
drift-guard · wafer fixtures (1) · `wafer build` · wasm-pack web · generator ·
CLI (re-quotes "x;y" when switching to ';') · Playwright page + query deep-link (2).

> Original work only — no competitor copy, branding, or trademarks copied.
