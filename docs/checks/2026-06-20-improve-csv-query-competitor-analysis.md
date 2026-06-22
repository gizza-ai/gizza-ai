# csv-query — competitor analysis (2026-06-20)

Thirty-fourth `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text tool,
all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| "SQL over CSV" web tools (q, csvkit, dsq) | full SQL incl. joins/group-by, in-browser or CLI | capabilities |
| query-CSV utilities | SELECT/WHERE/ORDER/LIMIT, projection | capabilities |

## Gap diff vs our tool
Our tool: SELECT <cols|*> [WHERE <col op value>] [ORDER BY <col> [ASC|DESC]]
[LIMIT n] — projection, single-condition filter (numeric-aware + contains),
numeric-aware sort, and limit. A coherent query interface that no single existing
tool offers (it composes select+filter+sort+limit). Requires a header.

**In-model gaps considered, deferred (the row says "and aggregate"):**
- **Aggregates / GROUP BY** (SELECT sum(x) ... GROUP BY g) — intentionally
  delegated to the dedicated csv-group-by and csv-pivot tools; documented in the
  skill description. Adding them here would reimplement those.
- **Multiple WHERE conditions (AND/OR)** — same single-condition limit as
  csv-filter; a shared grammar upgrade is the follow-up.
- **Column aliases / computed selects** (SELECT a+b AS c) — overlaps
  csv-formula-eval.

**Out-of-model:** JOINs across files (csv-merge concatenates; a real join is a
larger feature), full SQL engine.

## Tested
unit (6: SELECT *, project columns, WHERE numeric, contains+ORDER BY DESC+LIMIT
combined, ORDER BY string ASC, errors for empty/no-SELECT/unknown-col/bad-WHERE/
bad-LIMIT) + drift-guard · wafer fixtures (1) · `wafer build` · wasm-pack web ·
generator · CLI (SELECT+WHERE+ORDER+LIMIT → Carol,40) · Playwright page.

> Original work only — no competitor copy, branding, or trademarks copied.
