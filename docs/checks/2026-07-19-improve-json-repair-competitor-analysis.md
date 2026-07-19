# json-repair — competitor analysis (2026-07-19)

Scan done BEFORE implementation (create-next-tool step: competitor scan). One WebSearch
("JSON repair tool online fix malformed JSON trailing commas single quotes unquoted keys"),
top 3 reachable competitors skimmed. Paraphrased only — no competitor copy or branding.

## Competitors reviewed

1. **jsonlint.com/json-repair** — repairs 7 categories: trailing commas, single quotes,
   unquoted keys, missing commas, `//` + `/* */` comments, markdown ```` ```json ```` wrappers,
   truncated JSON (closes missing brackets). UI: input textarea + output + one repair button,
   plus a row of "try an example" preset buttons (one per fix category). States the tool fixes
   syntax, not semantics.
2. **jsonwebtools.com/json-repair** — same list plus: Python literals `True`/`False`/`None` →
   `true`/`false`/`null`, JS `undefined` handling, raw control characters in strings escaped
   (`\n`, `\t`), unclosed strings closed. Before/after example per category. Stated limits:
   can't fix severely truncated documents with multiple missing sections, semantic
   contradictions, mixed encodings, or JSON buried in prose without clear boundaries.
3. **jsontotable.org/json-fixer** — trailing commas, single quotes, missing commas, unquoted
   keys, bracket/brace mismatches, unescaped quotes in strings. UI: auto-fix button, sample
   loader, error highlighting with line numbers (server-backed; warns on large files).

## Table stakes → decision (in-model / out-of-model)

| Capability | Tag | Where it landed |
|---|---|---|
| Remove trailing commas | in-model | core parser (objects + arrays) |
| Single quotes / backticks / smart quotes → double quotes | in-model | core `string()` (accepts `'` `` ` `` `“”` `‘’`) |
| Quote unquoted object keys | in-model | core `object()` bare-key path |
| Insert missing commas (objects + arrays) | in-model | core separator tolerance |
| Strip `//` and `/* */` comments | in-model | core `skip_junk()` |
| Strip markdown ```` ```json ```` fences | in-model | core `strip_fence()` pre-pass |
| Complete truncated JSON (close strings/arrays/objects at EOF) | in-model | core EOF tolerance in every parser |
| Python `True`/`False`/`None` → `true`/`false`/`null` | in-model | core keyword table (case-insensitive; also `nil`) |
| `undefined` / `NaN` / `Infinity` / `-Infinity` → `null` | in-model | core keyword + signed-word path |
| Escape raw control chars (newline/tab) inside strings | in-model | kept raw in the parsed value; serde_json escapes on output |
| Quote unquoted string values (`{a: hello}`) | in-model | core unquoted-value fallback (reads to `,`/`}`/`]`/EOL) |
| Bracket/brace mismatch (`[1,2}`) | in-model | array accepts `}` as closer, object accepts `]` |
| Newline-delimited top-level values | in-model | wrapped into one array (matches repair-library convention) |
| Indent choice for the output (2/4/tab/minified) | in-model | `indent` enum param (default `2`) |
| Preset example buttons per fix category | in-model | `[[example]]` chips on the page (5 chips) |
| Unescaped interior quotes (`"he said "hi""`) | out-of-model | reliable detection needs backtracking heuristics; the parser ends the string at the first unescaped closer. Stated on the page under limits. |
| Error highlighting w/ line numbers in a live editor | out-of-model | the page platform renders a single text output, no code editor |
| AI/LLM-assisted semantic repair | out-of-model | gizza is deterministic local wasm; syntax-only by design (also a competitor caveat) |
| Server-side large-file processing | out-of-model | everything runs in-browser/CLI; no server. Depth cap 200 stated instead. |
| "What was fixed" diff report | out-of-model | single-text output surface; the output IS the repaired JSON |

No table-stake was dropped silently: everything above is either in the descriptor/core or in
this out-of-model list.

## Design decisions

- Params: `json` (string, required, multiline on page) + `indent` (`Param::enumv` `["2","4","tab","minify"]`,
  default `"2"`, friendly labels via `[input.labels]`). Matches json-beautify's indent concept but as a
  fixed-choice enum so the page renders a `<select>`.
- Object key order preserved (`serde_json` `preserve_order`), duplicate keys: last value wins
  (stated on the page).
- Depth cap 200 nesting levels (wasm stack safety) — exact boundary unit- and Playwright-tested
  (200 ok, 201 errors).
- Repair strategy = tolerant recursive-descent parse to a `serde_json::Value`, then re-serialize —
  guarantees the output is always valid JSON (never a partially-patched string).
- Keyword literals only count when followed by a delimiter, so `{a: true story}` repairs to the
  string `"true story"`, not `true` + garbage.
