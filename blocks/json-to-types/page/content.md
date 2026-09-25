## Infer code types from a real JSON sample

Use this JSON to types generator when an API gives you example payloads but no typed client model. Paste one representative JSON object, an array of objects, or a primitive sample and choose TypeScript, Rust, Go, or Python. The tool infers nested objects, arrays, optional keys, nullable values, integers versus floats, and mixed primitive unions where the target language can express them.

The output is deterministic and local. It preserves the JSON key order it sees, merges array element shapes, emits nested types once when the same shape repeats, and sanitizes field names that are not valid identifiers. For Rust and Go it can add serde/json annotations so the generated names still map back to the original JSON keys. For TypeScript it emits interfaces; for Rust and Go it emits structs; for Python it emits `@dataclass` classes.

Choose **Null and missing fields** based on how strict your downstream model should be. **Optional** is the practical default: a field that is `null` or missing from some array items becomes optional (`?:`, `Option<T>`, pointer, or `Optional[T]`). **Nullable** keeps fields required when they are present but lets the type include null. **Required** ignores nullability and missing-key hints, which is useful when you know the sample is incomplete noise rather than a schema guarantee.

### Worked example

Paste this JSON, keep **Output language** as TypeScript, and set **Root type name** to `User`:

```json
{"id":1,"name":"Ada","email":null,"tags":["admin"],"profile":{"active":true}}
```

The generated output starts with a public root interface and a nested profile type:

```ts
export interface User {
  id: number;
  name: string;
  email?: null;
  tags: string[];
  profile: Profile;
}
```

Switch to Rust, Go, or Python to generate equivalent models for a backend client. If a JSON key is not a legal identifier, the code uses a safe field name and, where the language supports it, an annotation that keeps the original JSON key for serialization.

### Limits and edge cases

- Input is capped at 2 MB and 64 levels of nesting.
- This is inference from examples, not a full schema validator. A sample cannot prove that a field never appears with another type unless that case is present.
- Heterogeneous arrays are merged. Compatible numbers widen from integer to float; incompatible object shapes become optional fields; incompatible primitive kinds become unions where possible or `any`/generic values where not.
- Struct/class names are derived from field names and PascalCased automatically. Repeated identical object shapes are deduplicated and reused.
- The output is meant as a starting point. Review names and optionality before committing generated code to a production API client.

## FAQ

<details>
<summary>Is this the same as generating types from JSON Schema?</summary>

No. JSON Schema is an explicit contract, while this tool infers a best-effort model from example JSON values. It is faster when all you have is a sample response, but it cannot know constraints that are absent from the sample, such as string formats, enum value sets, minimums, maximums, or whether a currently missing field is actually required.

</details>

<details>
<summary>How are optional and nullable fields decided?</summary>

When an array contains multiple objects, a key that appears in only some objects is treated as optional. A key whose value is `null` is also optional under the default strategy. The nullable strategy keeps explicit nulls in the type while reserving optional markers for missing keys. The required strategy forces all fields to be required and drops null-only hints.

</details>

<details>
<summary>What happens to keys like `user-id`, `class`, or `displayName`?</summary>

Generated field names are sanitized for the target language. TypeScript can quote unusual property names, Rust emits snake_case fields with `#[serde(rename = "...")]` when annotations are enabled, Go emits exported names with `json:"..."` tags, and Python avoids reserved words by adding a suffix and noting the original key in metadata where appropriate.

</details>

<details>
<summary>Can I use the generated Rust or Go structs directly for serialization?</summary>

Usually yes after review. Keep **Include JSON/serde annotations** enabled to preserve original JSON key names. Rust output includes serde derives and rename attributes; Go output includes `json` struct tags and `omitempty` for optional fields. You may still want to move nested types, rename fields, or replace generic values with domain-specific enums.

</details>

<details>
<summary>Why did a field become `any`, `serde_json::Value`, `interface{}`, or `Any`?</summary>

That means the samples had incompatible value kinds that the target language cannot express precisely without a custom union. For example, if one array item has a field as an object and another has the same field as a number, the safe generated type falls back to the language's generic JSON-like value.

</details>
