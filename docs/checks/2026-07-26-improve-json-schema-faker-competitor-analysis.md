# json-schema-faker — competitor analysis (2026-07-26)

Tool: **json-schema-faker** — generate N rows of realistic fake data that conform to a
JSON Schema, respecting string formats (email, uuid, date-time, …). Pure Rust, deterministic.

## Competitors skimmed (top 3)

1. **json-schema-faker (JS/npm, json-schema-faker.js.org)** — the canonical library. Combines
   JSON Schema with a faker backend. Supports the full Draft 2019-09/2020-12 surface: `$ref`,
   `oneOf`/`anyOf`/`allOf`, nested objects/arrays, `pattern` (AST regex → matching string),
   `enum`/`const`, all string `format`s (email, uuid, date-time, uri, ipv4/ipv6, hostname, …),
   numeric `minimum`/`maximum`/`multipleOf`, `minLength`/`maxLength`, `minItems`/`maxItems`.
   Options: `useDefaultValue`, `alwaysFakeOptionals`, `optionalsProbability`, `fixedProbabilities`,
   `minItems`/`maxItems` global overrides, and an injectable `random`/seed for reproducibility.
2. **jsf (Python, pypi.org/project/jsf)** — "create fake JSON files from a JSON schema".
   Out-of-the-box generation, extendable custom data providers, and multi-level state for
   *dependent* data. `generate(n)` produces N samples. Honors formats + constraints.
3. **jsondatafaker (Python)** — realistic synthetic JSON; schema defined in YAML; N records;
   aimed at test-data generation.

(Search: json-schema-faker.js.org, npmjs.com/package/json-schema-faker, pypi.org/project/jsf,
pypi.org/project/jsondatafaker — paraphrased, no copy/branding reused.)

## Table-stakes → in-model / out-of-model

| Capability | Decision | Notes |
|---|---|---|
| N records / count | **in-model** | `count`, 1–1000 cap |
| Reproducible seed | **in-model** | `seed`; 0 = random per call (chat/web), deterministic core |
| Pretty vs compact | **in-model** | `pretty` bool |
| Output as array / NDJSON / CSV | **in-model** | `output` enum json/jsonl/csv |
| Types string/integer/number/boolean/array/object | **in-model** | recursive |
| `enum` / `const` | **in-model** | const wins, then enum |
| `required` | **in-model** | every defined property is generated, so required is always satisfied |
| String formats email/uuid/date/date-time/uri/ipv4 | **in-model** | the picked set |
| `minLength`/`maxLength` | **in-model** | plain (format-less) strings |
| `minimum`/`maximum` | **in-model** | integer + number |
| `minItems`/`maxItems` | **in-model** | array length |
| `additionalProperties` | **in-model (ignored/false honored)** | never invents extra keys |
| `$ref` | **out-of-model → hard error** | no resolver; fail rather than mis-generate |
| `oneOf`/`anyOf`/`allOf`/`not` | **out-of-model → hard error** | schema-combination not modeled |
| `patternProperties`/`dependencies`/`if`-`then`-`else` | **out-of-model → hard error** | |
| `pattern` (regex → string) | **out-of-model → hard error** | assertion we can't satisfy; refuse, don't ignore |
| formats ipv6/hostname/regex/… | **out-of-model (graceful)** | `format` is an annotation, not an assertion — falls back to a plain string that still conforms |
| `multipleOf` | **out-of-model (graceful)** | annotation-adjacent; not enforced (documented) |
| custom providers / dependent-data state | **out-of-model** | listed, not built |

Design rule honored: every unsupported **assertion** (`$ref`, combinators, pattern,
patternProperties, dependencies, conditionals) is a hard error with a clear message — never
silently ignored. Unsupported **annotations** (unknown formats) fall back to a value that still
conforms to the schema.
