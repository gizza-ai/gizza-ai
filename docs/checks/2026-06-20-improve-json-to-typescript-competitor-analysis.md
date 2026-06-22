# json-to-typescript — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/json-to-typescript` — infer TypeScript interfaces from a JSON
sample. Chat + CLI + page (pure-text field inputs; serde_json + a type-inference
walker, no extra deps).

## Why this (and not the JSON↔CSV picks)

The adjacent backlog items `json-csv-converter` and `json-to-csv` were skiplisted
as full duplicates of the existing `csv-json-convert` (already bidirectional with
`flatten`). `json-to-typescript` is genuinely new.

## What competitors do

- **transform.tools / quicktype / json2ts sites** — paste JSON, get types.
  quicktype is the gold standard (many languages). Weaknesses: most run on a
  server (your JSON is uploaded), and the simple ones don't merge array elements
  (so they miss optional fields) or don't name nested interfaces well.
- **quicktype CLI** — excellent but needs Node install + flags.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: page runs
   in-browser, CLI headless, and it works in the chat Service Worker. The JSON
   never leaves the device.
2. **Merges array elements for accurate optionality.** Across an array of
   objects, a key present in only some elements is marked **optional** (`?`), and
   keys with differing types become **unions** (`number | string`) — the thing
   naive converters get wrong.
3. **Named nested interfaces.** Nested objects are emitted as their own
   interfaces named from the field (deduped on collision), not inlined — cleaner,
   reusable output.
4. **Correct edge handling.** `null` → `null`, empty arrays → `any[]`,
   non-identifier keys are quoted (`"first-name": string`), a top-level array
   yields `export type Root = RootItem[];`, union element arrays are parenthesized
   (`(number | string)[]`).
5. **Configurable.** `root_name` names the top type; `export` toggles the
   `export` keyword.
6. **Three surfaces + deep-links.**

## Honest scope

- Infers from a **sample** (structural), like all such tools — it can't know a
  field is optional if your sample always includes it, nor infer string literal
  unions/enums.
- Emits `interface`s (+ a `type` alias for array/primitive roots); no JSDoc,
  branded types, or per-language output (TS only).

## Tests

8 core unit tests: simple object (string/number/boolean), nested object → own
interface, array with an optional + typed field, primitive union in an array
(`(boolean | number | string)[]`), nullable field, root array → `type` alias,
non-identifier key quoting, and invalid-JSON error. Plus the block drift-guard
schema test. CLI + Playwright (infer via fill; optional-field via deep-link)
verified — see commit.
