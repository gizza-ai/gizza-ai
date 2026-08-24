# yaml-query competitor analysis (2026-08-22)

Backlog tool: `yaml-query` — read, filter, and transform YAML with jq-style expressions, converting the result between YAML and JSON.

## Competitor scan

Search query: `online YAML jq query yq tool transform YAML JSON filter`.

### 1. Python yq / jq wrapper

Observed table stakes:
- Accept a jq filter as the primary input.
- Read YAML and transcode it into the jq data model.
- Emit JSON by default or YAML with an explicit YAML-output option.
- Support raw string output for shell pipelines.
- Handle multi-document YAML streams.

Fit decision:
- In model: jq-style filter, YAML input, JSON/YAML output, raw string output, multi-document modes.
- Out of model: file arguments and command-line in-place editing because gizza tools receive pasted values and return a result string.

### 2. mikefarah/yq CLI

Observed table stakes:
- jq-like expression language over YAML and JSON.
- Selection, projection, mapping, filtering, aggregation, and conversion.
- Multiple input formats and output formats.
- Multi-document resource streams for Kubernetes-style manifests.
- Common examples around `.services.web.ports`, `.spec.template.spec.containers[].image`, `select`, `map`, and `keys`.

Fit decision:
- In model: yq-like filter field, auto/yaml/json input selection, yaml/json output selection, each/slurp document handling, example presets for compose, Kubernetes, and multi-doc queries.
- Out of model: yq-specific source-preserving assignment, comment-preserving writes, style operators, and in-place file updates. The gizza model is pure compute over supplied text and the jaq engine operates on a data tree.

### 3. YAMLScript / ys query documentation

Observed table stakes:
- Query and transform YAML/JSON with a jq-adjacent workflow.
- Convert between YAML and JSON for piping to other tools.
- Worked examples for extracting keys and sections.
- Clear distinction between data querying and broader language/runtime features.

Fit decision:
- In model: transform data with a query string, output format control, compact/pretty JSON control, worked examples.
- Out of model: full scripting language evaluation and module loading. This tool stays deterministic and browser-local.

## Descriptor and UX decisions

Implemented in-model controls:
- `yaml` textarea for the source document.
- `query` text input for the jq/yq-style filter.
- `input_format` enum: `auto`, `yaml`, `json`.
- `output_format` enum: `yaml`, `json`.
- `documents` enum: `each`, `slurp`.
- `pretty` checkbox for indented JSON.
- `raw_output` checkbox for unquoted scalar strings.

Examples/presets cover:
- docker-compose port extraction.
- service names as compact JSON.
- Kubernetes container image extraction.
- slurped multi-document resource names.

Limits documented:
- 4 MiB input cap.
- 50,000 output value cap.
- comments and original formatting are not preserved after query transforms.
- complex YAML mapping keys are rejected because jq object keys must be strings.
