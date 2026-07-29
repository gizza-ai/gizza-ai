# Competitor analysis — openapi-to-typescript-types (2026-07-29)

Scan run **before** implementing. One WebSearch for "OpenAPI schema to TypeScript
types generator online tool"; skimmed the top real competitors. All notes are
**paraphrased** — no competitor copy, branding, or trademarks are reproduced.

## Competitors reviewed

1. **openapi-typescript** (openapi-ts.dev / npm) — the dominant OSS CLI/library.
   Turns a full OpenAPI 3.0/3.1 spec into a single `.d.ts` with `paths`,
   `components`, `operations` interface trees. Options include: `--export-type`
   (emit `type` instead of `interface`), `--immutable` (readonly), `--enum`
   (real TS enums instead of unions), `--alphabetize` (sort properties),
   `--additional-properties`, `--default-non-nullable`, `--empty-objects-unknown`.
   Reads JSON or YAML, local + remote `$ref`. Table-stakes reference for options.
2. **js2ts.com — OpenAPI to TypeScript** — free online paste box; OpenAPI/Swagger
   JSON or YAML in, TS interfaces out. Focuses on `schemas` → interfaces. Simple
   one-screen UX, copy button.
3. **GitLoop — OpenAPI to TypeScript Types** — online paste box, YAML or JSON in,
   "clean TS type definitions" out. Also schemas-focused, no client generation.
4. **AI Dev Hub — OpenAPI to TypeScript Generator** — online; generates
   interfaces/types **and** a fetch client from the spec; JSON or YAML input.
5. **openapi-typescript-codegen** (npm) — generates a full typed client
   (services + models) from the spec; heavier, install-required, model files per
   schema.

## Table-stakes parameters (with defaults) and fit

| Capability | Competitors | In/out of model | Our decision |
|---|---|---|---|
| JSON **and** YAML input | all | in-model | `input_format` = auto/json/yaml (default auto) |
| Convert `components.schemas` / `definitions` | all schema tools | in-model | core scope; supports both 3.x and 2.0 |
| `interface` vs `type` output | openapi-typescript (`--export-type`) | in-model | `declaration` = interface/type (default interface) |
| Real `enum` vs union | openapi-typescript (`--enum`) | in-model | `enum_style` = union/enum (default union) |
| `readonly`/immutable | openapi-typescript (`--immutable`) | in-model | `readonly` boolean (default false) |
| Alphabetize properties | openapi-typescript (`--alphabetize`) | in-model | `sort` boolean (default false) |
| Optional vs required control | codegen tools, discussions | in-model | `optional_style` = spec/optional/required (default spec) |
| `export` prefix toggle | common | in-model | `export` boolean (default true) |
| Indent width | most formatters | in-model | `indent` 0–8 (default 2) |
| `$ref` → named type | all | in-model | resolved for local `#/…` refs |
| `nullable` / 3.1 `["string","null"]` | openapi-typescript | in-model | both → `| null` unions |
| `allOf`/`oneOf`/`anyOf` | openapi-typescript | in-model | intersection / union |
| JSDoc from `description` | openapi-typescript | in-model | emitted for types + properties |
| Copy result button | online tools | in-model | provided by the generic page (Copy result) |
| Worked-example presets | online tools | in-model | 3 `[[example]]` preset chips |

## Considered, not built (out-of-model or out-of-scope)

- **Full client generation** (paths/operations → fetch client, services) —
  openapi-typescript, codegen, AI Dev Hub. Out of scope for a *types* tool and a
  much larger surface; the tool's stated job is schema → types. Documented as a
  limit on the page.
- **Remote/external `$ref` resolution** (fetch other files/URLs) — needs network
  + a bundler; gizza tools are browser-local and offline. We resolve local
  `#/…` refs and tell users to bundle first. Documented as a limit.
- **Validation keywords → runtime validators** (zod/io-ts style) — a different
  output shape and out of scope; TS types can't hold `pattern`/`minimum`. Noted
  in the FAQ.
- **Per-schema separate files / hoisting nested objects into named interfaces** —
  codegen emits a file per model; a single-output paste tool keeps nested objects
  inline. Documented as a limit.

## Result

Every table-stakes parameter landed in the descriptor. The tool matches the
online schema-converters on scope and adds the power-user knobs from
openapi-typescript (interface/type, enum style, readonly, sort, optional style,
indent) while staying a single-paste, browser-local tool. Larger features
(client generation, remote refs, validators) are listed as out-of-model, not
silently dropped.
