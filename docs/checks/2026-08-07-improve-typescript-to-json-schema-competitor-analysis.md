# typescript-to-json-schema — competitor analysis (2026-08-07)

Scope: convert a pasted TypeScript **interface / type alias / enum** into an equivalent
JSON Schema. Pure, in-browser, no upload. Findings are paraphrased summaries of publicly
observable capabilities — no competitor copy, wording, branding, or trademarks were reused.

## Top 3 scanned

| # | Competitor | Shape | Notable capabilities observed |
|---|-----------|-------|------------------------------|
| 1 | transform.tools — TypeScript → JSON Schema | Free browser playground, live two-pane editor | Runs a full TS-compiler-backed generator client-side; handles arbitrary source files, generics, imports-free single files; emits `definitions`/`$ref` for named types; picks a root type; no options UI beyond paste-and-go |
| 2 | terrific.tools — TypeScript to JSON Schema | Free browser converter page | Interfaces + type aliases, nested objects, arrays, optional properties, unions; browser-local (states nothing is sent to a server); copy/download output |
| 3 | vega/ts-json-schema-generator (npm/CLI, the engine behind most web playgrounds) | CLI + library | Full type-checker resolution, generics, utility types, `--type` root selection, `$ref` + `definitions`, JSDoc annotation levels (none / basic / extended), `additionalProperties` toggle, draft selection, `markdownDescription`, `@nullable` / `@asType` |

(YousefED/typescript-json-schema is the older, maintenance-mode sibling of #3 with a smaller
annotation set; it was folded into the #3 row rather than scanned separately.)

## Table stakes → in-model decisions

| Table stake | Decision | Where |
|---|---|---|
| `interface X { … }` and `type X = …` | **In** | parser handles both, plus bare anonymous type literals |
| Optional `?` properties → excluded from `required` | **In** | `required` list is source-ordered, non-optional props only |
| Primitives: `string`, `number`, `boolean`, `null` | **In** | plus `bigint`→`integer`, `any`/`unknown`→`{}`, `never`→`{"not":{}}`, `Date`→`string`/`date-time` |
| Arrays: `T[]`, `Array<T>`, `readonly T[]` | **In** | `{"type":"array","items":…}` |
| String / number literal unions → `enum` | **In** | all-same-primitive union collapses to `type` + `enum`; single literal → `const`; mixed → bare `enum`; non-literal members → `anyOf` |
| `enum X { A = "a" }` (incl. `const enum`, implicit numeric values) | **In** | lowered to a literal union |
| Nested object type literals | **In** | recursive, unlimited depth (capped at 64 to stop pathological input) |
| Named type references between declarations | **In** | `$ref` into `$defs` (2020-12) / `definitions` (Draft-07); only reachable defs are emitted; recursive types are safe |
| `interface X extends Y` and object intersections `A & B` | **In** | shallow member merge; non-object operands fall back to `allOf` |
| Index signatures `[k: string]: T` and `Record<string, T>` | **In** | become `additionalProperties: T` |
| Tuples `[A, B]` | **In** | `prefixItems`+`items:false` (2020-12) / `items` array + `additionalItems:false` (Draft-07) |
| Root type selection | **In** | `root_type` param; defaults to the first declaration |
| Draft choice (2020-12 / Draft-07) | **In** | `draft` param — changes `$schema`, the defs pointer, and tuple keywords |
| Strict `additionalProperties: false` toggle | **In** | `additional_properties` param, strict by default |
| JSDoc `/** … */` → `description` | **In** | on declarations and properties |
| JSDoc constraint annotations | **In** | `@format @pattern @minimum @maximum @minLength @maxLength @minItems @maxItems @default @title @example @deprecated @nullable @asType` (`@TJS-type` alias) |
| Pretty-printed, copy/downloadable output | **In** | page emits text output with the generic download link |
| Browser-local, nothing uploaded | **In** | WebAssembly, same as the rest of the toolkit |

## Deliberately out of scope (stated on the page, not silently dropped)

These need a real TypeScript type checker (a multi-megabyte JS compiler), which does not fit a
pure-Rust WebAssembly block:

- **Generics with type parameters** — `interface Box<T>` / `Foo<Bar>` instantiation.
- **Utility & mapped types** — `Partial`, `Pick`, `Omit`, `Required`, `Readonly`, `Exclude`,
  `keyof`, `typeof`, indexed access `T["k"]`, template literal types, conditional types.
- **Cross-file resolution** — `import` / `export … from`; paste the types you need into one input.
- **Functions, methods, classes, namespaces, decorators** — not expressible as JSON Schema data.
- **`markdownDescription`** — an editor-specific extension keyword; skipped in favour of plain
  `description`.

Each of these produces an explicit, line-numbered error naming the construct rather than a silent
wrong schema.

## Gap-closing done in this pass

- Added `root_type`, `draft`, `required`, `additional_properties`, `jsdoc` options (competitor #3
  parity; #1 and #2 expose none of these).
- `$defs`/`definitions` + `$ref` output with reachability pruning — matches #1/#3, absent in #2.
- JSDoc annotation support at the "extended" level of #3, including `@asType` and `@nullable`.
- Line-numbered rejection messages for unsupported TypeScript — none of the three scanned tools
  say *which* construct they could not handle.
- Page: three worked example chips, a full worked example (input **and** output), a stated limits
  section, and 5 FAQ accordions.
