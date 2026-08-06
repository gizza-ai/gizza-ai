# json-remove-nulls — competitor analysis (2026-08-06)

Scan run BEFORE implementation (one web search: "online tool remove null values from JSON
recursively remove empty keys"). All notes are **paraphrased observations** of what the tools
advertise — no competitor copy, branding, or markup was reproduced.

## Top 3 real tools reviewed

| # | Tool | What it offers (paraphrased) | Notable gaps |
|---|------|------------------------------|--------------|
| 1 | Flipper File — "Remove JSON null fields" (`flipperfile.com/developer-tools/remove-json-null-fields/`) | Paste or drop a file, pick cleanup settings, one action button. Recursive null removal at any depth. Optional toggles for empty strings and empty objects/arrays, plus alphabetical key sorting. States that processing is browser-local and nothing is uploaded. | Empty-collection handling is a single lumped toggle (arrays and objects together); no explicit control over whether array *elements* are compacted; no indent/minify control surfaced. |
| 2 | Forge JSON — "Clean JSON" (`forgejson.com/tools/clean-json`) | The broadest option set: nulls, empty strings, empty collections, default-valued fields and duplicates, plus whitespace trimming and key sorting. Each operation toggles independently and they compose in a single pass. | Very wide scope (dedupe, default-value stripping) — closer to a general normalizer than a focused pruner; nothing states how array holes are treated after removal. |
| 3 | Imagetoolhub — "Remove keys from JSON" (`imagetoolhub.com/json/tools/remove-keys`) | Removes keys *by name*, by list, or by wildcard pattern, recursively. Scans the document first and lists every unique key with occurrence count, value types and a sample value before you delete. Checkbox/preset selection UI. | Targets keys by *name*, not by *value* — it cannot express "drop whatever is null"; no empty-value semantics at all. |

(A frequently-surfaced fourth result is a library gist rather than a tool, and several hits are
vendor forum threads about DataWeave / API Connect / ADF doing this in a pipeline — evidence the
task is common, but not usable as tool competitors.)

## Table stakes (must ship)

1. Recursive removal of object keys whose value is `null`, at every depth including inside arrays. ✅
2. Independent opt-in toggles for empty strings, empty arrays, empty objects. ✅ (three separate
   booleans — Flipper's lumped "empty objects/arrays" switch is coarser than users need.)
3. Strict parse/validate with a precise error, never silent garbage out. ✅ (`invalid JSON: …` with
   serde's line/column.)
4. Pretty vs minified output. ✅ (`indent` 0–8; `0` minifies — same control shape as the sibling
   `json-sort` tool, for family consistency.)
5. Browser-local processing, no upload, no account. ✅ (wasm on the page, plus CLI/chat.)
6. Preserve every other valid JSON value exactly — `false`, `0`, `""` when not opted in, nested
   types, key order. ✅ (unit-tested: `false`/`0` are never treated as empty.)

## Decisions taken

- **Array compaction is an explicit `arrays` enum, not a boolean.** Every competitor is silent on
  what happens to a `null` sitting *inside* an array — the two behaviors (leave the hole, or
  compact the array) are genuinely different data contracts, and an enum (`compact` | `keep`)
  names both on the page instead of hiding one behind an unchecked box. Default `compact`, because
  a user asking to "remove nulls" almost never wants `[1, null, 2]` back unchanged; `keep` is there
  for positional/tuple-shaped arrays where indices are load-bearing.
- **Cascading removal is the defined semantics, bottom-up.** Pruning children first means that if
  `remove_empty_objects` is on, an object left empty *by the prune itself* also disappears (and can
  cascade upward). This is what "recursive" should mean and is the behavior competitors imply but
  don't specify; it is documented on the page and unit-tested.
- **`trim_strings` adopted from Forge JSON.** Cheap, composes cleanly with `remove_empty_strings`
  (`"  "` becomes removable), and is the one item from their wide option set that belongs in a
  pruner.
- **Key sorting NOT adopted** (Flipper, Forge both ship it): the repo already has `json-sort`,
  which does it better (asc/desc, case-insensitive, array sorting). Duplicating it here would be
  schema bloat — pipe the two tools instead.
- **Removing duplicates / default-valued fields NOT adopted** (Forge JSON): "default value" is
  schema-dependent and undefinable without a schema input; dedupe is a different tool's job.
- **Key-name / wildcard targeting NOT adopted** (Imagetoolhub): that is removal by *name*, an
  orthogonal tool shape, and `json-path-edit` already covers targeted deletion.
- **The root value is never dropped.** If the whole document prunes to `{}`, `{}` is returned
  rather than an empty string — an empty output would be invalid JSON.

## Out-of-model (considered, not built)

- File-drop / bulk multi-file cleanup and download-as-file pipelines (Flipper) — the page takes
  pasted text; a batch/file-queue UI is a platform feature, not a per-tool one.
- Pre-scan key inventory with occurrence counts and sample values (Imagetoolhub) — a rich
  interactive panel; the generated page is a form + output, and this tool prunes by value, not by
  a picked key list.
- Accounts, saved presets, server-side processing — outside the browser-local, no-account model.
