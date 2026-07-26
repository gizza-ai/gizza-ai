## About this tool

JSON Schema Faker generates synthetic records from a JSON Schema subset. It is
useful when you need repeatable fixtures for API tests, demos, import pipelines,
or validation examples, but you do not want to hand-write sample rows.

Paste a schema, set a **count** and **seed**, and choose JSON array, JSON Lines,
or CSV output. The same seed produces the same data, so failing tests are easy to
reproduce. Everything runs locally in the browser.

### Supported schema subset

- Root schemas should be objects when you want CSV; JSON and JSONL can also emit
  arrays or scalar roots.
- Supported types: `string`, `integer`, `number`, `boolean`, `array`, `object`,
  and `null`.
- Supported constraints: `properties`, `required`, `enum`, `const`,
  `minLength`, `maxLength`, `minimum`, `maximum`, `minItems`, and `maxItems`.
- Supported string formats: `email`, `uuid`, `date`, `date-time`, `uri`, and
  `ipv4`. Unknown formats fall back to a plain string because JSON Schema treats
  `format` as an annotation unless a validator opts in.
- Unsupported assertion constructs such as `$ref`, `oneOf`, `anyOf`, `allOf`,
  `pattern`, `patternProperties`, dependencies, and conditionals are rejected
  with a clear error instead of being silently ignored.

### Worked example

Schema:

```json
{
  "type": "object",
  "properties": {
    "id": { "type": "integer", "minimum": 1, "maximum": 99 },
    "email": { "type": "string", "format": "email" },
    "role": { "enum": ["admin", "user", "guest"] }
  },
  "required": ["id", "email"]
}
```

With `count = 3`, `seed = 42`, and `output = json`, the tool returns a JSON array
of three object records with numeric IDs, email-shaped strings, and roles chosen
from the enum.

## Limits & edge cases

- Record count is capped at **1000** to keep browser, chat, and CLI output
  bounded.
- CSV output requires object rows. Nested objects or arrays are serialized into a
  single CSV cell as compact JSON.
- The generator emits every defined property, so `required` fields are always
  present. It does not invent extra properties.
- `multipleOf`, custom faker providers, cross-field dependencies, and `$ref`
  resolution are intentionally out of scope for this pure local block.

## FAQ

<details>
<summary>Does the output really validate against my JSON Schema?</summary>

It conforms to the supported subset listed above. For unsupported assertion
keywords such as `$ref`, `oneOf`, `pattern`, or conditional schemas, the tool
returns an error rather than generating data that might fail validation later.

</details>

<details>
<summary>Why does the same seed produce the same rows?</summary>

The generator uses a deterministic pseudo-random sequence. That makes sample data
reproducible across the page, CLI, and chat block: keep the same schema, count,
seed, and output format to get the same result again.

</details>

<details>
<summary>Can I generate CSV from a nested schema?</summary>

Yes, as long as the root output is an object. Top-level properties become columns;
nested objects and arrays are JSON-serialized into individual cells so the CSV
remains valid and can round-trip through common spreadsheet tools.

</details>

<details>
<summary>What happens to unknown string formats?</summary>

Unknown formats are treated as annotations and generate a plain string. Known
formats such as `email`, `uuid`, `date`, `date-time`, `uri`, and `ipv4` get
format-shaped values.

</details>
