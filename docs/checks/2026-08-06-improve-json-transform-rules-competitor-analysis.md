# json-transform-rules competitor analysis (2026-08-06)

Backlog item: `json-transform-rules` — reshape JSON from a source structure to a target structure using declarative match/transform rules.

## Sources checked

- JSON Viewer Tool, "JSON Transform / Map Tool" (`jsonviewertool.com/json-transform`). Search result and page metadata describe browser-local JSON mapping with JSONPath queries, `target=JSONPath` rules, nested target paths, and format-oriented controls.
- BeameryHQ `json-ditto` (`github.com/BeameryHQ/json-ditto`). Search result describes a declarative JSON-to-JSON mapper that parses a mapping file and emits JSON matching an output definition.
- `jsonpath-mapper` (`github.com/neilflatley/jsonpath-mapper`). README describes JSON-to-JSON transformation templates, JSONPath-powered value resolution, mapping reusable API responses into domain objects, and hooks/functions for enrichment.

## Table-stakes capabilities and decisions

| Capability / UX pattern | Competitor signal | Decision for this tool |
| --- | --- | --- |
| Paste source JSON and a separate mapping/rules document | All checked tools center on an input JSON document plus mapping definitions. | In model. Page has large multiline `json` and `rules` fields; CLI/chat descriptor makes both required. |
| Quick `target = selector` mapping syntax | JSON Viewer Tool exposes `target=JSONPath` style rules. | In model. Shorthand lines support `target = $.path`, comments, constants, optional defaults with `?=`, and `-path` removal. |
| Nested target creation | JSON transform tools need target objects that differ from source shape. | In model. Target paths support dotted keys, quoted keys, indexes, and `[]` append. |
| JSONPath-style source selection | All three competitors use or describe JSONPath-like selectors. | In model with a deliberate safe subset: `$`, `.key`, `["key"]`, `[0]`, `[*]`, `.*`, and `..key`. Full JSONPath filters/slices/scripts are out of model. |
| Mapping file / reusable declaration | json-ditto and jsonpath-mapper emphasize reusable mapping templates. | In model. Rules can be shorthand, target-to-selector JSON objects, or JSON arrays of rule objects. |
| Defaults and missing-field behavior | Production mappers need predictable behavior when a path is absent. | In model. Rule-level `default` plus global `on_missing=skip|null|error`. |
| Transform functions / enrichment | jsonpath-mapper mentions enrichment/functions; online tools often include simple transformations. | In model for deterministic pure transforms: upper/lower/trim, type coercion, length, sort/reverse/unique/flatten, keys/values, count/sum/min/max/avg/first/last/join. Arbitrary user functions are out of model. |
| Per-item mapping over arrays | Common API-mapping use case: one source array to one target array. | In model. `each` selector runs all rules once per matched item. |
| Debugging output | Mapping rules are easy to get wrong, so a rule trace is useful. | In model. `output=report` shows source/target, matches, writes, missing/default/when counts. |
| Visual drag-and-drop mapper | Some commercial ETL/mapping products provide graphical field wiring. | Out of model for the current generic page generator. Preset examples, labels, and multiline editors are used instead. |
| Full JSONPath expression engine with predicates and scripts | Some libraries support richer JSONPath. | Out of model: predicates, slices, unions, arithmetic, and script callbacks would add engine complexity and security risk. The page states the supported subset. |
| Arbitrary JavaScript plugin transforms | jsonpath-mapper-style applications can call custom functions. | Out of model for sandbox portability. Only deterministic built-in transforms are shipped. |

## Descriptor / page choices

- Required params: `json`, `rules`.
- Optional controls: `each`, `mode`, `on_missing`, `array_mode`, `pretty`, `indent`, `output`.
- Enum params use select controls with labels; `indent` uses a slider; source JSON and rules are multiline.
- Example chips cover API response mapping, fan-out mapping, merge-mode redaction, and debug reports.
- Limits are documented explicitly rather than silently failing: 5 MB JSON, 200 KB rules, 500 rules, 100k selector matches, 50k `each` items, 10k target array growth.
