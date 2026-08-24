# elasticsearch-mapping-generator — competitor analysis (2026-08-22)

Scan run BEFORE implementing, per `/create-next-tool` step 4. All findings are paraphrased
from public documentation and product pages; no competitor copy, branding, or trademarks are
reproduced here or in the tool. "Elasticsearch" is used only as the factual name of the target
system whose mapping format the tool emits.

## Competitors skimmed

| # | Tool | Kind | Reachable |
|---|------|------|-----------|
| 1 | `toystars/elasticsearch-mapper` (GitHub, npm) | Library — mapping generator from a JSON document / MongoDB collection | yes |
| 2 | `mapping.karigar.io` — schema → mapping generator | Web tool | yes |
| 3 | `knowBalpreet/json-to-es-mapping` (+ its hosted app) | Web tool — paste raw JSON, edit mappings live | yes (repo; hosted page renders client-side, so its option set was read from the repo) |

Reference for correctness (not a competitor): the official Elasticsearch dynamic field mapping
rules, which define the baseline every one of these tools is judged against.

## What each one offers

**1. `elasticsearch-mapper`** — infers field types from a sample document and emits a mapping.
Per-field config array selects which fields are indexed/tokenized and which analyzer is used for
indexing vs searching (its defaults are an edge-ngram indexing analyzer and a whitespace search
analyzer). Nested objects are emitted as `object` with their own `properties`. Exposes
dynamic-mapping toggles at index and type level. Deliberately does **not** expose shard/replica
settings — mapping generation only.

**2. `mapping.karigar.io`** — inputs are a JSON **Schema** (not a raw document), a target
Elasticsearch major version, a language analyzer picked from a list (English, French, German,
Chinese, Japanese, Russian), and a three-way dynamic-mapping policy (reject new fields / allow
new fields unindexed / allow new fields and map them dynamically). Output is a mapping body.

**3. `json-to-es-mapping`** — the closest match to this row: paste a raw JSON document, get a
mapping instantly, edit it in a live editor, copy it out. Its selling point is speed and
editability rather than configurability — the repo exposes no option surface; type inference is
fixed in a single conversion module.

## Table-stakes checklist (derived) → where each landed

| Table stake | Seen in | Decision |
|---|---|---|
| Paste a raw JSON document → mapping | 1, 3 | **in** — `json`, the required input |
| Multiple sample documents merged (root array) | 1 (partly) | **in** — root arrays merge all documents |
| Recursive objects → `object` + `properties` | 1, 2, 3 | **in** |
| String → `text` + `.keyword` multi-field | 3, ES default | **in** — `text_fields` (default `text_keyword`), `ignore_above` (default 256) |
| `keyword`-only / `text`-only string strategy | ES practice | **in** — `text_fields = keyword \| text` |
| Date detection on ISO-8601 strings | ES default | **in** — `date_detection` (default true) |
| Numeric strings → numeric type | ES option | **in** — `numeric_detection` (default false, matching ES) |
| Integer/float width choice (`long`/`integer`/`short`, `float`/`double`/`half_float`) | 2 | **in** — `integer_type`, `float_type` |
| Arrays of objects as `nested` instead of `object` | 2 (schema-driven) | **in** — `array_objects` |
| Dynamic-mapping policy | 1, 2 | **in** — `dynamic` (`true`/`false`/`strict`/`runtime`) |
| Language / custom analyzer on text fields | 1, 2 | **in** — `analyzer` (free-form name, e.g. `english`) |
| Index settings (shards/replicas) around the mapping | none (1 explicitly declines) | **in** as an opt-in — `output = create-index`, `shards`, `replicas` |
| Output shape choice (mapping body vs bare properties) | implicit in all | **in** — `output = mappings \| create-index \| properties` |
| Live-editable output | 3 | **out of model** — the page renders read-only output with copy/download; editing happens in the user's editor |
| Elasticsearch major-version switch | 2 | **out of model as a switch** — the emitted body is the modern typeless form (ES 7+/8+/9+); pre-7 mapping-type wrappers are legacy and would double the output matrix. Stated in the FAQ. |
| Per-field manual type overrides in a config array | 1 | **out of model** — a per-field override table is not expressible in the single-form parameter surface; documented as "edit the generated mapping". |
| MongoDB collection input | 1 | **out of model** — no network/DB access in this toolkit. |

## Differentiators this tool ships that none of the three had

- **`ip` detection** (`detect_ip`) — IPv4/IPv6 strings map to the `ip` type instead of `text`.
- **`geo_point` detection** (`detect_geo_point`) — `{"lat": …, "lon": …}` objects map to
  `geo_point` instead of an `object` with two `float` fields.
- **Conflict widening across merged documents** — a field seen as both integer and float widens
  to the float type; genuinely incompatible observations widen to the string strategy rather
  than silently taking the first value (which is what the ES dynamic rules do for arrays).
- **Explicit `date_detection` / `numeric_detection` echoed into the mapping body**, so the
  generated mapping governs *future* dynamic fields the same way the sample was interpreted.
- Deterministic alphabetical property ordering, so two runs diff cleanly.

## UX patterns adopted

- Preset `[[example]]` chips (a typical document, a nested/geo document, and a strict
  create-index body) — competitors 2 and 3 both lean on instant results from a paste, so the
  page runs on load with a prefilled sample.
- `multiline = true` on the JSON field (competitors all use a paste area).
- Friendly `<select>` labels via `[input.labels]` for the enum values, since raw values like
  `text_keyword` are not self-explanatory.
- `kind = "slider"` is **not** used: `ignore_above`, `shards` and `replicas` are exact values
  users type, not values they scrub for.

## In-model vs out-of-model summary

Everything in the table-stakes list above is either implemented or explicitly listed as
out-of-model with a reason. Nothing was dropped silently.
