# data-reshape — competitor analysis (2026-07-23)

Tool function: parse JSON, YAML, CSV, or XML; query/filter/aggregate the data; optionally template it into a new structured output. Paraphrased notes only — no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **JSONata Exerciser / JSONata Studio** — interactive JSONata expression editors over JSON. Table-stakes: expression field, input data textarea, instant output, object/array construction, path navigation, predicates, functions such as `$sum`/`$count`, and visible syntax/evaluation errors.
2. **jq / yq online playgrounds** — query and transform JSON/YAML from pasted text. Table-stakes: format-aware parsing, expression/query textbox, pretty output, examples/presets, and clear parse errors.
3. **CyberChef-style data pipelines** — multi-format conversion and extraction recipes. Table-stakes: paste input, choose operations/format, convert/query without upload, downloadable/copyable output, and examples for CSV/JSON/XML transforms.
4. **CSV/JSON/XML converter tools** — support CSV header-row parsing, JSON/YAML output, XML-to-object mapping, and basic type coercion so numbers aggregate as numbers rather than strings.

## Table-stakes → in-model / out-of-model decisions

| Capability | Competitors | Decision |
|---|---|---|
| Query language that can both select and construct output | JSONata tools, jq/yq | **in** — JSONata expression param; supports navigation, filtering, aggregates, and object/array construction. |
| JSON input | all | **in** — direct JSON parse. |
| YAML input | yq/playgrounds | **in** — parse YAML into the shared JSON value model. |
| CSV input with header row | converter/pipeline tools | **in** — parse to an array of row objects; coerce numeric/boolean/null-like cells. |
| XML input | xq/converter tools | **in** — reuse existing XML-to-JSON core with coercion. |
| Output JSON and YAML | jq/yq/converters | **in** — JSON default, YAML option. |
| Pretty-print toggle | playgrounds/converters | **in** — `pretty` checkbox for JSON; YAML is naturally block style. |
| Auto-detect input format | converters | **in** — predictable sniffing for JSON/XML/CSV/YAML. |
| Worked examples/preset chips | playgrounds | **in** — CSV aggregate, JSON reshape, CSV filter, YAML, XML examples. |
| Streaming very large files | CLI data tools | **out** — page/chat model is pasted text, not streaming files; keep examples and limits to moderate inputs. |
| Full jq/yq language compatibility | jq/yq | **out** — this tool is JSONata-based; existing `jq-query`/`jsonata-query` cover single-format specialist workflows. |
| Multi-step visual recipe builder | pipeline GUIs | **out** — one expression keeps the generic gizza page simple. |

## UX/control decisions

- Use two textareas: source `data` and JSONata `query`.
- Use enum selects for `input_format` (`auto`, `json`, `yaml`, `csv`, `xml`) and `output_format` (`json`, `yaml`).
- Use a checkbox for `pretty` and preset chips for the common examples.
- Show exact output text and errors in the standard pure-tool output area.

## Distinction from existing blocks

This is not a replacement for `jsonata-query`, `jq-query`, `jsonpath-query`, `csv-query`, or format converters. Those are single-format or conversion-only tools. `data-reshape` combines multi-format parsing with one JSONata query/template step and returns JSON/YAML output, filling the "reshape across JSON/YAML/CSV/XML" workflow without requiring several separate tools in sequence.
