# json-structure-analyzer — competitor analysis (2026-07-22)

Tool function: parse a JSON document and report its *shape* without transforming it — max
nesting depth, node counts by type, key-frequency ranking, per-path type distribution (array
indices collapsed to `[]`), array-length stats, raw-vs-minified byte size, and quality warnings.
Pure compute; runs in-browser via WebAssembly and via the CLI/chat.

## Competitors skimmed (paraphrased)

1. Browser JSON viewers / tree formatters: paste JSON, get a collapsible tree with syntax
   highlighting, validation, and a beautify/minify toggle. Strong on visual browsing and error
   position, but they show the *data*, not aggregate structural statistics (depth, key counts,
   per-path type spread).
2. JSON-to-Schema inference utilities: read a sample document and emit a JSON Schema (types,
   required keys, nested definitions). Close in spirit — they generalize types per path — but the
   output is a schema to reuse, not a human-readable structural audit, and they usually skip
   frequency/size/warning summaries.
3. Query/stat CLIs (jq-style filters, JSONPath explorers, "json stats" one-liners): powerful for
   extracting values or counting a specific path, but the user must already know what to ask; they
   don't give a single overview pass over an unknown document.

## Table-stakes → decision

| Capability | Competitors | Our decision |
|---|---|---|
| Valid-JSON parse with clear error + position | all viewers | **in-model** — parse error surfaces `serde_json` position |
| Max nesting depth | some viewers/schema tools | **in-model** — root = depth 0, each container +1 |
| Node counts by type | schema tools | **in-model** — objects/arrays/strings/numbers/booleans/nulls + totals/empties |
| Key-frequency ranking | rare | **in-model differentiator** — every key occurrence counted, ranked |
| Per-path type distribution (indices → `[]`) | schema inference | **in-model differentiator** — surfaces mixed-type fields |
| Array-length stats (count/min/max/avg) | rare | **in-model** — one pass over all arrays |
| Raw vs minified size + compression potential | formatters (minify) | **in-model** — reported as stats, no rewrite |
| Quality warnings (deep nesting, recurring keys, empties) | rare | **in-model differentiator** |
| Output as JSON *or* human text | some CLIs | **in-model** — `format` enum (json default / text) |
| Cap long lists | large-doc tools | **in-model** — `top_keys` / `top_paths`, `0` = all, with truncation flag |
| Interactive collapsible tree UI | viewers | **out-of-model** — declarative page has no tree canvas; listed, not built |
| Emit a reusable JSON Schema file | schema tools | **out-of-model** — this tool audits shape, doesn't generate a schema artifact |
| Live edit / re-format the document | formatters | **out-of-scope** — a separate beautify/minify tool owns transformation |

## Defaults and verification notes

- Defaults (`format=json`, `top_keys=30`, `top_paths=50`) match the descriptor and manifest.
- `top_keys=0` / `top_paths=0` list every entry (no truncation flag); non-zero caps set the
  `*_truncated` flag. Boundary values are exercised by the page test.
- Text mode renders the same data as a plain-text report ("Max depth", "Key frequency", "Arrays",
  "Warnings"); JSON mode is pretty-printed and easy to diff or pipe onward.
- Path normalization collapses array indices so all elements of an array share one path, which is
  what makes a mixed-type field (e.g. `["number","string"]` at `$.items[].price`) visible in one
  line — the main edge over per-value viewers.
- Page copy stays generic: no competitor brand names or copied marketing text.
