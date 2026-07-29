## About this tool

Paste an **OpenAPI 3.x** or **Swagger 2.0** document and get back TypeScript type
declarations for every schema it defines. The generator reads the
`components.schemas` object (OpenAPI 3.x) or the top-level `definitions` object
(Swagger 2.0), then walks each schema into a matching TypeScript type — no
install, no server, no account. Everything runs locally in your browser.

It understands the JSON-Schema constructs OpenAPI relies on: `$ref` (rendered as
a reference to the named type), `enum` and `const`, `nullable` and the OpenAPI
3.1 `type: ["string", "null"]` form (both become unions), `required` (drives the
`?` optional marker), `properties` and `additionalProperties` (index
signatures), array `items` and tuple `prefixItems`, and `allOf` / `oneOf` /
`anyOf` (intersection and union types). Schema `description`s become JSDoc
comments.

### Worked example

Input (YAML):

```yaml
openapi: "3.0.3"
components:
  schemas:
    Status:
      type: string
      enum: [active, banned]
    User:
      type: object
      required: [id, name]
      properties:
        id: { type: integer }
        name: { type: string }
        status: { $ref: "#/components/schemas/Status" }
        tags: { type: array, items: { type: string } }
```

Output (default settings — `interface` declarations, `union` enums):

```ts
export type Status = "active" | "banned";

export interface User {
  id: number;
  name: string;
  status?: Status;
  tags?: string[];
}
```

`id` and `name` are required so they have no `?`; `status` and `tags` are not in
`required`, so they are optional. `Status` is emitted as a string-literal union;
switch **String enums as** to `enum` to get a real `export enum Status { … }`
instead.

### Options

- **Input format** — `auto` (try JSON, then YAML), or force `json` / `yaml`.
- **Object schemas as** — `interface` (`export interface X { … }`) or `type`
  (`export type X = { … }`). Schemas that are not plain objects (a bare string,
  an enum, a union) are always emitted as `type` aliases, since an `interface`
  can only describe an object.
- **String enums as** — `union` for a `"a" | "b"` string-literal union, or
  `enum` for a real TypeScript `enum`.
- **Property optionality** — `spec` honors the schema's `required` array;
  `optional` marks every property `?`; `required` marks none.
- **export / readonly / sort** — prefix declarations with `export`, mark every
  property `readonly`, and/or alphabetize properties.
- **Indent** — spaces per nesting level, 0 to 8.

## FAQ

<details>
<summary>Which part of the OpenAPI document does it convert?</summary>

Only the reusable schema objects: `components.schemas` for OpenAPI 3.x, or the
top-level `definitions` for Swagger 2.0. It does **not** generate a request
client from `paths` / operations, and it does not read `parameters`,
`requestBody`, or `responses` inline schemas — put your models under
`components.schemas` (the standard place) and reference them with `$ref`.

</details>

<details>
<summary>How are optional and nullable properties handled?</summary>

**Optional** and **nullable** are two different things in OpenAPI, and this tool
keeps them separate. A property is *optional* (`name?: string`) when it is not
listed in the schema's `required` array — that is the `Property optionality:
spec` default. A property is *nullable* (`name: string | null`) when the schema
sets `nullable: true` (OpenAPI 3.0) or uses `type: ["string", "null"]` (OpenAPI
3.1). A field can be both, neither, or either.

</details>

<details>
<summary>Does it resolve $ref and external files?</summary>

Local `$ref`s like `#/components/schemas/User` are resolved to the matching
TypeScript type name, so your types stay cross-linked. **External** `$ref`s
(another file or URL, e.g. `./common.yaml#/Address`) are **not** fetched — only
the last path segment is used as the type name, and no declaration is emitted for
it. Bundle your spec into a single document first (with a tool like
`swagger-cli bundle`) if it splits schemas across files.

</details>

<details>
<summary>What TypeScript does an empty or free-form object become?</summary>

An object schema with no `properties` becomes a `Record<…>`: `Record<string,
unknown>` for a plain or `additionalProperties: true` object, `Record<string,
T>` when `additionalProperties` is itself a schema, and `Record<string, never>`
when `additionalProperties: false`. An object that has properties **and**
`additionalProperties` gets both the named fields and an `[key: string]: …`
index signature.

</details>

<details>
<summary>What about validation keywords like pattern, minimum, or format?</summary>

TypeScript's type system can't express runtime constraints, so JSON-Schema
validation keywords (`pattern`, `minimum`/`maximum`, `minLength`, `multipleOf`,
`format`, …) are ignored — a `string` with a `format: email` is still `string`.
The output describes the *shape* of your data, not its runtime validity. If you
need runtime validation, generate a schema-aware validator separately.

</details>

## Limits and edge cases

- Converts `components.schemas` (3.x) or `definitions` (2.0) only — not `paths`,
  operations, or inline request/response schemas.
- External and remote `$ref`s are not fetched (local `#/…` refs are resolved).
- Validation keywords (`pattern`, `minimum`, `format`, …) are dropped —
  TypeScript can't represent them.
- Nested inline objects stay inline (`{ … }`); they are not hoisted into their
  own named interfaces.
- A schema with no recognizable `type` and no `properties` becomes `unknown`.
- Indent is clamped to 0–8 spaces.
