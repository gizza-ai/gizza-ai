## Convert TypeScript types to JSON Schema in your browser

Paste a TypeScript `interface`, `type` alias, `enum`, or a bare object type and get a pretty-printed JSON Schema. The converter is built for common API and form-validation models: primitives, arrays, tuples, optional properties, string/number literal unions, nested objects, local type references, `extends`, object intersections, index signatures, `Record<string, T>`, and JSDoc descriptions/constraints.

Everything runs locally in WebAssembly. Your source types are not uploaded, and the output is plain JSON you can copy into validators, OpenAPI schemas, tests, or documentation.

### Worked example

Input:

```ts
/** A user visible in the admin UI. */
interface User {
  id: number;
  email: string;
  role: "admin" | "editor" | "viewer";
  bio?: string;
}
```

With **Root type** set to `User`, **Schema draft** set to `2020-12`, **Mark non-optional members required** on, and **Allow extra properties** off, the result is an object schema with `id`, `email`, `role`, and `bio` under `properties`, `role` as a string enum, `required: ["id", "email", "role"]`, `additionalProperties: false`, and the JSDoc sentence as the schema description.

### Controls

- **TypeScript source** — paste declarations in one file. A bare type literal such as `{ id: string; tags?: string[] }` also works.
- **Root type** — choose which declaration becomes the top-level schema. Leave blank to use the first declaration.
- **Schema draft** — emit Draft 2020-12 (`$defs`, `prefixItems`) or Draft-07 (`definitions`, tuple `items` array).
- **Mark non-optional members required** — when enabled, properties without `?` or `| undefined` are listed in `required`.
- **Allow extra properties** — when disabled, object schemas get `additionalProperties: false`; index signatures and `Record<string, T>` still emit their value schema.
- **Use JSDoc comments** — prose becomes `description`; common tags such as `@format`, `@pattern`, `@minimum`, `@maximum`, `@minLength`, `@maxLength`, `@default`, `@example`, `@deprecated`, `@nullable`, and `@asType` become JSON Schema keywords.

### Limits and edge cases

This is a focused converter, not a full TypeScript compiler. It deliberately rejects constructs that need real type checking or cross-file resolution: generics, utility/mapped types such as `Partial` or `Pick`, `keyof`, `typeof`, indexed access, conditional types, imports, exports from other files, functions, methods, classes, namespaces, and decorators. Unsupported input returns a line-numbered error naming the construct instead of guessing.

References between declarations in the same paste are supported and emitted as `$ref` plus reachable `$defs`/`definitions`. Recursive local references are allowed. `any` and `unknown` become open schemas, `never` becomes an impossible schema, `Date` becomes a `string` with `format: date-time`, and `bigint` becomes `integer`.

## FAQ

<details>
<summary>Is this the same as a TypeScript compiler based generator?</summary>

No. Compiler-based generators can resolve imports, generics, utility types, and complex inferred types. This tool intentionally handles a practical single-file subset that is small enough to run as a pure WebAssembly block. When a type needs the compiler, the tool says so rather than emitting a misleading schema.

</details>

<details>
<summary>How are optional properties handled?</summary>

A property marked with `?` is omitted from the `required` array. A property whose type includes `undefined` is treated the same way. If you turn off **Mark non-optional members required**, the tool omits the `required` array entirely.

</details>

<details>
<summary>Can I choose Draft-07 instead of JSON Schema 2020-12?</summary>

Yes. Pick `draft-07` in the schema draft control. The converter switches the `$schema` URL, uses `definitions` instead of `$defs`, and emits Draft-07 tuple keywords (`items` array plus `additionalItems: false`) instead of 2020-12 `prefixItems`.

</details>

<details>
<summary>What JSDoc tags become schema keywords?</summary>

Plain JSDoc text becomes `description`. Tags including `@title`, `@format`, `@pattern`, numeric/string/array bounds such as `@minimum` and `@maxLength`, `@default`, `@example`, `@deprecated`, `@nullable`, and `@asType` are mapped to matching JSON Schema keywords when **Use JSDoc comments** is enabled.

</details>

<details>
<summary>Why did a TypeScript type fail to convert?</summary>

The error message should include a line number and the unsupported construct. The most common causes are generics (`Box<T>`), utility types (`Partial<User>`), imports, methods, functions, or mapped/conditional types. Paste the resolved shape as an interface or object type literal to convert it here.

</details>
