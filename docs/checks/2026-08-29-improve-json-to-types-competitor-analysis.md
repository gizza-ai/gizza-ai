# json-to-types — competitor analysis (2026-08-29)

Scan run before finishing `blocks/json-to-types`, focused on tools that infer code types from a pasted JSON sample. Observations are paraphrased from public pages; no competitor copy, branding, or trademarks is reused.

## Competitors reviewed

| # | Tool | What it is |
|---|------|------------|
| 1 | uitweak JSON converter | Multi-language JSON-to-model generator covering TypeScript, Go, Rust, Python and other targets |
| 2 | PureDevTools JSON to Python dataclass | Browser-local Python dataclass generator with nested classes and optional fields |
| 3 | dev-workshop JSON to types | JSON sample to TypeScript, Go and Python types with deterministic output |
| 4 | Chrome Web Store JSON to Types extension | Browser extension advertising TypeScript/Go free and additional languages including Python/Rust |
| 5 | shard.tools JSON to Python | Browser-local JSON-to-Python dataclass converter |

## Table stakes observed

| Capability | 1 | 2 | 3 | 4 | 5 | Our decision |
|---|---|---|---|---|---|---|
| Paste one JSON sample | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — required `json` field, object/array/primitive accepted |
| Nested object types | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — nested object fields create named nested types |
| Arrays of objects | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — element shapes merge into one inferred type |
| Multiple target languages | ✅ | Python-only | ✅ | ✅ | Python-only | **In-model** — `output_language=typescript/rust/go/python` |
| Root type/class name | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — `root_name`, PascalCased automatically |
| Optional / missing fields | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — missing array-object fields become optional |
| Nullable handling controls | ✅ | partial | ✅ | partial | partial | **In-model** — `optional_strategy=optional/nullable/required` |
| Serialization annotations / tags | ✅ | partial | ✅ | ✅ | partial | **In-model** — `json_annotations` controls serde/json tags where relevant |
| Export/public types | ✅ | — | ✅ | ✅ | — | **In-model** — `export` toggles TypeScript export/Rust pub/Go exported names |
| Identifier sanitizing | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — invalid/reserved keys are renamed or quoted safely |
| Browser-local/no upload | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — pure Rust/WASM, no network |
| Download `.ts`/`.go`/`.py` files | ✅ | ✅ | — | extension flow | ✅ | **Out-of-model / unnecessary** — copyable text output and CLI stdout are the repository-native surfaces |
| Rich editor, formatting settings, package/module wrappers | ✅ | ✅ | partial | ✅ | partial | **Mostly out-of-model** — this tool emits model definitions, not full project scaffolds |
| JSON Schema or OpenAPI import | some adjacent tools | — | — | — | — | **Out-of-model** — a schema contract is a different input shape; this tool infers from examples |

## In-model controls adopted

- A multiline JSON field for the sample, because every competitor starts with a paste area.
- A target-language select rather than separate tools for each language; this avoids duplicating `json-to-typescript` while adding Rust, Go and Python coverage.
- `root_name` for the generated top-level type, since API users rarely want a literal `Root` name in their code.
- `optional_strategy` so users can choose practical optional fields, explicit nullable unions, or strict required output.
- `json_annotations` for Rust serde attributes and Go `json` tags.
- `export` for public versus local TypeScript/Rust/Go declarations.
- Preset examples for TypeScript, Rust, Go and Python.

## Gaps deliberately closed

- The same local engine is available on the CLI, page and chat surfaces.
- The inference is deterministic: key order is preserved and repeated object shapes are emitted once.
- Arrays with heterogeneous object elements merge fields instead of throwing away less common keys.
- Primitive conflicts produce a target-language-safe union when possible and a generic JSON-like type when not.

## Out-of-model (listed, not built)

| Feature | Why it does not fit |
|---|---|
| Importing JSON Schema/OpenAPI | Those are explicit schemas with `$ref`, allOf/oneOf, formats and constraints; this block infers from values only |
| Full package/project generation | Module files, package manifests and build integration are language/project-specific scaffolding |
| Download buttons per extension | The generic page already offers copyable text; CLI stdout covers file redirection |
| Runtime validation code | This tool emits static model shapes, not validators or parsers |
| Enum inference from many samples | A few sample string values are not enough to prove a closed enum safely |
| Date/time/email semantic typing | JSON carries strings only; semantic formats require heuristics that can be wrong |
