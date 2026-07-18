## About this tool

**JSON to JSON Schema** infers a practical JSON Schema from one pasted JSON
example or from an array of examples. It is useful when you have sample API
responses, config objects, event payloads, or webhook fixtures and want a
starting schema without writing every property by hand.

- **Objects and arrays are inferred recursively**: nested objects become
  `properties`, arrays become `items`, and arrays with several objects merge the
  observed item shapes.
- **Required keys are data-driven**: keys present in every merged object are
  listed under `required`; keys missing in some examples become optional.
- **Mixed values become unions**: for example `[1, "two"]` produces an item type
  that accepts both `integer` and `string`.
- **String formats are detected** when enabled: email, URI, date-time, date,
  UUID (Draft 2020-12), and IPv4.
- **Strict by default**: `additionalProperties: false` is emitted unless you turn
  on “Allow extra properties”.

Everything runs locally in your browser through WebAssembly. Your sample JSON is
not uploaded.

### Worked example

Input:

```json
[{ "id": 1, "email": "ada@example.com" }, { "id": 2, "email": "grace@example.com", "admin": true }]
```

Output excerpt:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "admin": { "type": "boolean" },
      "email": { "type": "string", "format": "email" },
      "id": { "type": "integer" }
    },
    "required": ["email", "id"],
    "additionalProperties": false
  }
}
```

### Limits & edge cases

- This is inference, not proof. A small sample may miss optional fields, allowed
  enum values, minimums, maximums, string lengths, or domain-specific rules.
- Empty arrays infer `items: {}` because there are no elements to learn from.
- Incompatible shapes (for example sometimes an object and sometimes a string)
  are intentionally widened rather than guessed narrowly.
- It does not validate future JSON against the schema; use the generated schema
  in your validator of choice.

## FAQ

<details>
<summary>Can I infer from more than one example?</summary>

Yes. Paste a JSON array of example objects. The tool merges the item schemas:
properties seen in every object stay required, while properties missing from at
least one object become optional.

</details>

<details>
<summary>Why did a field become optional?</summary>

A property is required only when it appears in every merged object at that level.
If one sample omits it, the generated schema still includes the property type but
leaves it out of the `required` array.

</details>

<details>
<summary>What does “Allow extra properties” change?</summary>

By default the schema is strict and emits `additionalProperties: false` on
objects. Turning this option on omits that keyword, allowing validators to accept
keys that were not present in the sample.

</details>

<details>
<summary>Which JSON Schema drafts are supported?</summary>

The tool emits either Draft 2020-12 (default) or Draft-07. Draft choice changes
the `$schema` URI; UUID format detection is emitted only for Draft 2020-12 because
Draft-07 did not define a `uuid` format.

</details>

<details>
<summary>Does this replace reviewing the schema manually?</summary>

No. It creates a solid starting point from observed data, but you should still
review business rules such as enumerations, numeric bounds, string lengths,
patterns, and fields that did not appear in your examples.

</details>
