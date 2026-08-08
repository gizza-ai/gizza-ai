# Competitor analysis: yaml-path-query (2026-08-08)

Tool: `yaml-path-query` — query or edit YAML values by a path expression while preserving the rest of the file.

## Competitor scan

| Competitor / reference | Table-stakes behavior observed | In-model decision |
| --- | --- | --- |
| `yamlpath` CLI / Python library | Dot-style and slash-style paths, get/set workflows, typed value handling, output formatting choices, validation/check options, and preservation-oriented YAML edits. | Build the core get/set/delete path workflow, typed YAML values and preservation-oriented edits. Leave validation/check rules, merge/diff/scan workflows and alternate slash syntax out of model for a compact browser tool. |
| `yq` YAML processor examples | Query, add, update and delete YAML nodes from the command line; array indexes and nested paths are common. | Include query/set/delete modes, mapping keys, array indexes and missing-branch creation for set. Do not attempt the full expression language, pipes, filters or file-in-place command flags. |
| Online YAML editors / YAML Path Finder style tools | Paste YAML, inspect paths, copy a path, and receive immediate browser feedback. | Provide a local textarea page, path input, mode selector, JSON/YAML output selector and example chips. Do not include a tree viewer/path picker because the current generated page model is form-first. |

## Required capabilities mapped to the gizza model

- Path forms: `server.host`, `items[0].name`, `items.0.name`, quoted bracket keys such as `["my.key"]`, and an optional leading `$`. These are in-model and implemented.
- Modes: query, set and delete. These are in-model and implemented as `Param::enumv` choices.
- Typed set values: parse the set value as YAML so numbers, booleans, nulls, lists and inline maps keep their YAML types. Implemented; quoted strings force string output.
- Output formats: raw/YAML and pretty JSON. Implemented as an enum.
- Preservation: exact YAML comment preservation is difficult without a full round-trip AST. The implemented compromise uses parser source markers for verified scalar/new-key/delete splices and falls back to normalized re-emission when a safe splice is not possible. This keeps data correct while documenting formatting limits.
- UX controls: textarea for YAML input and set value, text path input, select controls for mode and output format, and example chips for read/update/list-item cases. Implemented in page metadata.

## Out-of-model / deliberately excluded

- Full JSONPath/YAMLPath expression language: wildcards, filters, recursive descent, slices, predicates and functions are broad query-language features. They are not needed for the single-node edit use case and would overlap existing JSONPath-style tools.
- Multi-document stream editing: safely choosing which document to edit requires another selector. The tool rejects multi-document YAML rather than guessing.
- File in-place editing and bulk scan/diff/merge workflows: those are CLI-suite features, not a pure local block/page operation.
- Tree-view path picker: useful in an app UI, but current generated pages are declarative form pages. Example chips cover common starting points.

## Verification examples used

- Query scalar: `server.host` over a nested document returns `localhost`.
- Query list item: `items[1]` can be rendered as JSON.
- Set scalar: `server.port` with value `9090` returns the document and preserves nearby comments when the edit can be spliced.
- Delete key/list item: removes the target and leaves siblings intact.
