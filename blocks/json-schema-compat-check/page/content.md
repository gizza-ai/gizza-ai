## About the JSON Schema compatibility checker

Paste the schema your system accepts today and the schema you want to ship next.
The checker compares common JSON Schema draft-7 validation keywords and reports
whether the change is safe for:

- **Consumers / backward compatibility** — the new schema still accepts data that
  was valid under the old schema. Tightening a schema can break this direction.
- **Producers / forward compatibility** — data produced for the new schema still
  fits consumers that validate against the old schema. Widening a schema can
  break this direction.
- **Both directions** — useful for API contracts, event streams, configuration
  files, and stored documents where readers and writers may deploy at different
  times.

The report is intentionally conservative. It proves straightforward type, enum,
const, required-field, object-property, array-item, numeric-bound, string-bound,
and `additionalProperties` changes; it emits warnings for composition keywords
and regular-expression changes that require a full schema solver to prove.
Everything runs locally in your browser.

### Worked example: adding a required field

Old schema:

```json
{"type":"object","required":["id"],"properties":{"id":{"type":"string"},"email":{"type":"string"}}}
```

New schema:

```json
{"type":"object","required":["id","email"],"properties":{"id":{"type":"string"},"email":{"type":"string"}}}
```

Result: the new schema is **breaking for consumers** because old valid records
could omit `email`. It is usually safe for producers because new records that
include `email` still satisfy the old schema, unless your old consumers reject
unknown or newly required fields in application code outside JSON Schema.

### What the checker compares

| Keyword family | Examples | Direction-aware behavior |
| --- | --- | --- |
| Primitive type | `type`, `enum`, `const` | Narrowing breaks consumers; widening breaks producers. |
| Object shape | `required`, `properties`, `additionalProperties` | Added required fields and removed accepted properties are consumer risks; removed required fields and newly accepted properties are producer risks. |
| Numeric bounds | `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`, `multipleOf` | Tighter ranges break consumers; wider ranges break producers. |
| String bounds | `minLength`, `maxLength`, `pattern` | Length changes are classified; pattern edits are warnings unless unchanged. |
| Arrays | `items`, `minItems`, `maxItems`, `uniqueItems` | Item and size constraints are compared recursively where they are single-schema forms. |
| Metadata | `title`, `description`, `default`, examples | Ignored because these do not change validation. |

### Limits and edge cases

- Input schemas are capped at 1 MiB each and must be valid JSON objects or
  booleans. Empty input and invalid JSON are errors.
- The checker follows local JSON Pointer `$ref`s in the same document when they
  are simple and acyclic. External references are warnings because they cannot be
  fetched in this offline tool.
- `allOf`, `anyOf`, `oneOf`, `not`, conditional schemas, pattern properties, and
  dependent schemas are reported as warnings when changed instead of being
  treated as safe.
- Reports are capped at 200 findings so a large schema diff stays readable.
- JSON Schema compatibility is not the same thing as application compatibility:
  custom validators, database migrations, generated types, and business rules may
  add breaking changes outside the schema file.

### FAQ

<details>
<summary>What is the difference between consumer and producer compatibility?</summary>

Consumer compatibility asks whether a reader upgraded to the new schema can still
read old data. Producer compatibility asks whether data written for the new
schema can still be read by systems that have not upgraded and still validate
against the old schema. Tightening a schema usually threatens consumers; widening
one usually threatens old producers or readers.

</details>

<details>
<summary>Is this the same as a formal JSON Schema subtype proof?</summary>

No. It is a practical keyword-level checker for the validation rules most teams
change in API contracts and event schemas. Some JSON Schema features interact in
ways that require a solver to prove exactly, so this tool emits warnings for
those changes instead of pretending they are safe.

</details>

<details>
<summary>Why are regular expression changes warnings?</summary>

Determining whether one regular expression accepts a subset of another is much
harder than comparing a numeric minimum or an enum set. A changed `pattern` may
be safe, narrowing, or widening depending on the expressions. The checker points
it out so a human can review it.

</details>

<details>
<summary>Can I use it for OpenAPI request and response schemas?</summary>

Yes, if you paste the actual JSON Schema object for the request body, response,
or component you want to compare. For an OpenAPI operation, check inputs and
outputs separately: request-body compatibility affects clients as producers;
response compatibility affects clients as consumers.

</details>

<details>
<summary>Does it upload my schemas?</summary>

No. The comparison runs in WebAssembly in your browser, and the CLI uses the same
local Rust code. External `$ref` URLs are not fetched; they are reported as
warnings instead.

</details>
