# api-response-diff — competitor analysis (2026-08-16)

Scan run BEFORE finalising the block. One WebSearch
(`compare two JSON API responses ignore fields timestamps ids online diff tool`)
plus a skim of the top real competitor tools. No competitor copy, wording, or
branding was reused — only the *capability set* was compared, and every
capability below was re-described from scratch.

## Competitors reviewed

| # | Tool | What it does | Notable |
|---|------|--------------|---------|
| 1 | JSONBlitz Compare (`jsonblitz.dev/compare`) | Semantic JSON diff in the browser | Key-order-insensitive; **arrays matched by id**; a free-form **ignore list** (`timestamp`, `requestId`, `updatedAt`) that re-runs the diff instantly; **wildcard patterns** (`*.updatedAt`) matching a field at any depth; explicit in-browser privacy claim |
| 2 | Apify JSON Diff Tool (`apify.com/automation-lab/json-diff-tool`) | Hosted actor that diffs two JSON documents or two live endpoints | **Ignore-keys list** taking both bare names and dotted paths (`meta.requestId`, `_etag`); fetches **URL A vs URL B** (staging vs production); scheduled runs + webhook alerting |
| 3 | Daily Developer Tools — JSON Diff (`dailydevelopertools.com/json-diff.html`) | Local browser JSON diff | Order-insensitive by default; separate toggles for **ignore keys**, **ignore array order**, **ignore timestamps**, **ignore UUIDs**, "ignore volatile keys" — i.e. value-*shape* filters, not just name filters |
| 4 | ArrayDiff — API Response Diff (`arraydiff.com/api-response-diff`) | Response-diff framing of a JSON diff | Positions the tool around REST debugging, API **version comparison** (`/v1` vs `/v2`) and webhook-payload validation; output is a flat added / removed / modified field list |
| 5 | json-compare guide (`json-compare.json-format.com`) | Buyer's guide for API-response comparison | Confirms the category's core requirement: responses carry dynamic `timestamp` / `request_id` / generated ids, so a comparator must let you **configure fields to ignore** or the diff is unusable |

## Table stakes → decisions

| Capability | Competitors | Decision |
|---|---|---|
| Structural (not textual) diff, path-addressed | 1, 2, 3, 4 | **Built** — recursive walk emitting `$.data.items[2].name` paths with kinds `added` / `removed` / `changed` / `type_changed`. `type_changed` is a level of detail none of the five report separately. |
| Ignore list by field name | 1, 2, 3, 5 | **Built** — `ignore`, comma- *or* newline-separated. A bare name matches that key at **any depth**, matching what 1 and 3 do by default. |
| Ignore by dotted path | 2 | **Built** — a pattern containing `.` or `[` is anchored at the root (`data.token`, `$.data.token`), so a deep unrelated `token` is untouched. |
| Wildcard / glob patterns | 1 | **Built** — `*_at` inside a segment, `*` for one segment, `**` for any number, `[2]` / `[*]` for array indices. Superset of the single `*.field` form seen in the wild. |
| Ignore timestamp-shaped values | 3 | **Built** — `ignore_timestamps` matches ISO-8601 dates/datetimes and epoch seconds/milliseconds on **both** sides, so a timestamp→null shape change is still reported. |
| Ignore UUID-shaped values | 3 | **Built** — `ignore_uuids`, canonical 8-4-4-4-12 only (deliberately not "any hex blob", which would swallow hashes and tokens). |
| Ignore array order | 1, 3 | **Built** — `array_match=set` pairs elements as a multiset; leftovers are reported as added/removed. |
| Match array elements by id | 1 | **Built** — `array_match=key` + `array_key` (any field, not just `id`). Paths read `$.items[id=a1].price`. Falls back to index matching with a `notes` entry when the key is missing or repeats, instead of failing. |
| Numeric tolerance | — | **Built** — none of the five offer it; rounding/aggregation drift between two backends is the most common false positive after timestamps. |
| String leniency (case, whitespace) and cross-type equality (`"5"` = `5`) | — | **Built** — `ignore_case`, `trim_strings`, `coerce_types`; `null_equals_missing` for the null-vs-absent split that serialisers change between versions. |
| Show what was ignored | — | **Built** — `counts.ignored` + `ignored_paths`. Competitors drop ignored fields silently, which makes an over-broad pattern indistinguishable from "no change". |
| Machine-readable output | 2, 4 | **Built** — default `output=report` is a full JSON object (`equal`, `counts`, `notes`, `ignored_paths`, `changes`), usable as a contract-test assertion. |
| Readable one-line-per-change view | 1, 3, 4 | **Built** — `output=summary` (`~ $.data.total: 2 -> 3`). |
| JSON Patch export | — | **Built** — `output=patch` emits RFC 6902 `add`/`remove`/`replace`. Requires `array_match=index`, because JSON Pointer positions stop being well-defined once elements are paired by key or as a set; the tool errors with that explanation rather than emitting wrong pointers. |
| Runs locally, responses never uploaded | 1, 3 | **Built (copy)** — true here (wasm in-page, CLI local, no network capability in the descriptor); stated in the hero, body copy and FAQ. |
| Deep-linkable / shareable configuration | — | **Already shipped** — the tool page reads every param from `?query=`. |

## Deliberately not built (out of model / already covered)

- **Fetching URL A vs URL B** (2) — needs network + auth headers, which this block
  deliberately does not declare. The existing `web-fetch` tool covers the fetch step
  and its output pipes straight into `left` / `right`.
- **Scheduled runs and webhook alerts** (2) — hosted-service behaviour, not a pure
  compute block.
- **Side-by-side coloured highlighting** (1, 4) — the page surface renders a text
  result; `output=summary` gives the same information as `+ - ~ !` markers that
  survive being pasted into a terminal, an issue or a CI log.
- **Schema-aware / OpenAPI-driven comparison** — a different tool shape; the
  existing `json-schema-batch-validate` and `json-to-json-schema` blocks cover the
  schema side.

## Not a duplicate

- `blocks/json-diff` is the plain structural diff: `left`, `right`, `indent` and
  nothing else — no ignore list, no shape filters, no array key/set matching, no
  tolerance, no summary or patch output. It answers "what differs"; this tool
  answers "what differs **that I care about**", which is the whole reason the
  API-response comparison category exists (5).
- `blocks/jwt-claims-diff`, `blocks/http-headers-diff`, `blocks/sbom-diff` and
  `blocks/column-value-diff` each diff one fixed document shape; none takes
  arbitrary JSON with volatile-field suppression.
