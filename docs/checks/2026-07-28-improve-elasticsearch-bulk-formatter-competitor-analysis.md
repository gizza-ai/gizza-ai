# Competitor analysis — elasticsearch-bulk-formatter (2026-07-28)

Tool function: turn a JSON **array of documents** into the newline-delimited JSON (NDJSON)
request body the Elasticsearch `_bulk` API expects — an action/metadata line optionally followed
by a source line, per document, ending with a trailing `\n`. Pure, browser-local, no network.

All competitor notes below are **paraphrased** for feature/UX ideas only — no copy, branding, or
trademarks reproduced.

## Sources skimmed

1. **Elastic official `_bulk` API reference** (elastic.co/guide … docs-bulk) — the authoritative
   format spec, i.e. the table-stakes any generator must match.
2. **`mradamlacey/json-to-es-bulk`** (GitHub CLI utility) — reads a JSON array file, emits a
   `_bulk` body file.
3. **`jq` recipe** (community, widely-cited: `jq -c '.[] | {"index":{"_index":"..."}}, .'`) — the
   de-facto ad-hoc "tool" people reach for.
4. **Elasticsearch MCP `bulk` tool** (glama.ai / awesimon) — programmatic bulk helper with an
   `idFieldName` option.

## Table-stakes format rules (from the Elastic spec — must all be honored)

- Four actions: `index` (create-or-replace), `create` (only if absent), `update` (partial),
  `delete` (remove).
- Action/metadata line: `{ "<action>": { "_index": …, "_id": …, … } }`. `_index` optional when
  the caller targets `/<index>/_bulk`; `_id` optional for index/create, **required for
  update/delete**.
- Source line: required for `index`/`create` (the document) and `update` (`{"doc": …}`, optionally
  `"doc_as_upsert": true`); **delete has NO source line**.
- Every line is compact (NOT pretty-printed — literal `\n` is the delimiter), and the body **must
  end with a trailing newline**.

## Competitor capabilities → our decision

| Capability | Seen in | In-model? | Decision |
| ---------- | ------- | --------- | -------- |
| Array-of-objects → `_bulk` NDJSON | all | yes | core behavior |
| Target index name → `_index` | json-to-es-bulk (`--index`), jq | yes | `index` param (omit when blank → URL-scoped) |
| Per-document `_id` pulled from a field | json-to-es-bulk (`id`), MCP (`idFieldName`) | yes | `id_field` param; value → `_id`, field stripped from source |
| Choice of action | Elastic spec (jq/json-to-es-bulk do index only) | yes | `action` enum index/create/update/delete (beats both concrete tools, which are index-only) |
| `update` → `doc` wrapper + `doc_as_upsert` | Elastic spec | yes | `update` action wraps `{"doc":…}`; `doc_as_upsert` boolean |
| Trailing newline + compact lines | Elastic spec | yes | always enforced (correctness, not an option) |
| Mapping `_type` (`--type`) | json-to-es-bulk | **rejected** | removed in ES 8+ (types gone); shipping it would emit bodies modern ES rejects |
| `_routing`, `version`, `retry_on_conflict` per-doc | Elastic spec | out-of-model (this pass) | niche per-doc concurrency/routing metadata; not offered by the mainstream generators; listed, not built |
| Stream/chunk to 5–15 MB batches, POST to a live cluster | curl/jq pipelines | out-of-model | needs network + a running cluster; browser-local tool only builds the body |

## UX ideas adopted (original implementation)

- Preset example chips (index / update-upsert / delete) so the worked example is one click.
- `id_field` strips the id from the emitted source (a document shouldn't repeat its own `_id`) —
  matches how json-to-es-bulk / the MCP `idFieldName` behave.
- Clear, actionable errors: non-array root, non-object item, and the update/delete-needs-`_id`
  rule are surfaced with the offending item index.
