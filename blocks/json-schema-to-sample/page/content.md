## About this tool

This tool turns a **JSON Schema** into the smallest JSON document that satisfies
it. Paste a schema, and you get an example instance you can drop straight into
API documentation, a test fixture, a mock response, or a request body you are
about to send by hand.

It is deliberately *minimal*, not random: by default only **required** properties
appear (plus any property that declares a `default`), every value is derived from
the schema itself, and the same schema always produces the same output. Nothing
is uploaded — the schema is parsed and the sample built inside your browser.

Tick **Include optional properties** when you want the full shape instead, raise
**Array items** to show more than one entry per array, and untick **Pretty-print**
for a single compact line.

### Where each value comes from

Values are taken from the first of these the schema provides, in order:

1. `const` — the only value the schema allows.
2. `default` — the documented fallback value.
3. `examples[0]`, or OpenAPI 3.0's singular `example`.
4. `enum[0]` — the first accepted choice.
5. A value generated from `type` and `format`.

So a schema that already documents itself with `default`/`examples` gets *your*
values back, not invented ones.

### Supported schema keywords

- **Types:** `object`, `array`, `string`, `integer`, `number`, `boolean`, `null`,
  union types such as `["string", "null"]` (the first non-null member wins), and
  the boolean schemas `true` / `false`.
- **Objects:** `properties`, `required`, `additionalProperties` as a schema.
- **Arrays:** `items`, `prefixItems`, draft-07 tuple `items: [...]` with
  `additionalItems`, `minItems`, `maxItems`, `uniqueItems`.
- **Strings:** `minLength`, `maxLength`, and `format` — `email`, `uuid`, `date`,
  `date-time`, `time`, `duration`, `uri`, `uri-reference`, `uri-template`,
  `hostname`, `ipv4`, `ipv6`, `byte`, `json-pointer`, `regex`. Unknown formats
  fall back to a plain string, because `format` is an annotation unless a
  validator opts in.
- **Numbers:** `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum` (both
  the draft-04 boolean form and the numeric form), `multipleOf`.
- **Composition:** `allOf` is deep-merged; `oneOf` / `anyOf` use the first branch.
- **References:** local `$ref` pointers into the same document —
  `#/$defs/Name`, `#/definitions/Name`, or any JSON pointer such as
  `#/properties/user`. Recursive schemas stop at the first repeat with `null`.

Drafts 07, 2019-09 and 2020-12 all work; the tool reads the keywords that are
present rather than requiring a `$schema` declaration.

### Worked example

Schema:

```json
{
  "type": "object",
  "properties": {
    "id": { "type": "integer", "minimum": 1 },
    "email": { "type": "string", "format": "email" },
    "nickname": { "type": "string" }
  },
  "required": ["id", "email"]
}
```

Sample JSON (defaults: optional properties off, one array item, pretty-printed):

```json
{
  "id": 1,
  "email": "user@example.com"
}
```

`nickname` is optional, so it is left out. `id` becomes `1` because `minimum: 1`
is the smallest allowed integer, and `email` is shaped by its `format`.

Tick **Include optional properties** and `nickname` joins the sample as
`"string"`.

## Limits & edge cases

- The schema must be at most **1 MB**, and generation stops after **50,000**
  values — lower **Array items** or untick **Include optional properties** if you
  hit that on a very wide schema.
- **Array items** accepts **0–50**. A larger `minItems` always wins, and
  `maxItems` still caps the result, so bounded arrays stay valid.
- Length assertions beat `format`: with `minLength`/`maxLength` set, the string is
  padded or truncated even if that spoils an email or UUID shape, because length
  is an assertion while `format` is an annotation.
- Recursive `$ref` chains are cut at the first repeat and nesting stops after 32
  levels; both emit `null` at the cut point rather than looping forever.
- Remote `$ref`s (`https://…/schema.json#/…`) are rejected with a clear error —
  there is no network fetch in a browser-local tool. Inline your definitions
  under `$defs` first.
- Not synthesised, and reported rather than faked where it matters: `pattern`,
  `patternProperties`, `not`, `if`/`then`/`else`, `dependentSchemas`,
  `dependencies`, `propertyNames`, `contains`, `minProperties`/`maxProperties`,
  and `readOnly`/`writeOnly` filtering. A schema whose validity rests only on
  `pattern` may need a manual touch-up.
- A schema of `false` accepts no instance at all, so it returns an error instead
  of a sample.

## FAQ

<details>
<summary>Will the generated sample actually validate against my schema?</summary>

For the keyword subset listed above, yes — required properties are present,
enums and `const` are respected, and numeric, length and item bounds are honored.
The exceptions are the keywords listed under limits, chiefly `pattern`: a plain
string cannot be synthesised to match an arbitrary regular expression, so run the
result through a validator if your schema leans on `pattern` or conditional
subschemas.

</details>

<details>
<summary>How is this different from a fake-data generator?</summary>

A faker invents plausible-looking rows and usually varies them per run. This tool
produces one *minimal* and *deterministic* instance: the same schema and options
always return byte-identical JSON, and values come from the schema's own `const`,
`default`, `examples` and `enum` before anything is generated. That is what you
want for documentation snippets, golden files and fixtures that must not churn in
version control.

</details>

<details>
<summary>Why are optional properties missing from my sample?</summary>

By design — a minimal instance only needs the properties listed in `required`.
Tick **Include optional properties** to emit every property in `properties`. One
exception applies either way: a property that declares a `default` is always
included, since the default is part of the effective document.

</details>

<details>
<summary>Does it follow $ref, allOf and oneOf?</summary>

Local `$ref` pointers are resolved against the same document, including
`#/$defs/Name`, `#/definitions/Name` and ordinary JSON pointers, and keywords
sitting next to a `$ref` are merged over the target. `allOf` branches are
deep-merged into one effective schema; `oneOf` and `anyOf` use the first branch,
which is the conventional choice for documentation examples. Remote `$ref`s are
not fetched.

</details>

<details>
<summary>How many items does an array get?</summary>

The **Array items** control, which defaults to 1. `minItems` raises it when it is
larger, `maxItems` lowers it, tuple positions from `prefixItems` are always all
emitted, and `uniqueItems` makes repeated entries distinct so the array stays
valid.

</details>
