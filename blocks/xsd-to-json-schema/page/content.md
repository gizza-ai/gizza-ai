## Convert XML Schema to JSON Schema locally

XSD and JSON Schema describe the same basic job — validating structured data —
but they use very different models. This converter reads a pasted `.xsd` file and
emits a JSON Schema document you can use with Draft 2020-12 or Draft-07
validators. It handles the common XSD building blocks: global elements, named
simple and complex types, attributes, sequences, choices, occurrence counts,
facets, documentation annotations and recursive references.

### Worked example

Input XSD:

```xml
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:element name="order">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="total" type="xs:decimal"/>
      </xs:sequence>
      <xs:attribute name="currency" type="xs:string" use="required"/>
    </xs:complexType>
  </xs:element>
</xs:schema>
```

Output includes an object schema for `order`, a required numeric `total`, a
required `@currency` string property, and `additionalProperties: false` unless
you turn on the relaxed option.

### What is mapped

- `xs:element`, `xs:complexType`, `xs:sequence` and `xs:all` become object
  properties in document order.
- `xs:choice` becomes a JSON Schema `oneOf` over the chosen members when required
  derivation is enabled.
- `xs:attribute use="required"` becomes a required property, prefixed with `@` by
  default so attributes do not collide with child elements.
- `xs:simpleType` facets become JSON Schema keywords such as `enum`, `pattern`,
  `minLength`, `maxLength`, `minimum`, `maximum` and `multipleOf`.
- Named global types become `$ref` entries in `$defs` (Draft 2020-12) or
  `definitions` (Draft-07), pruned to the selected root.

### Limits

The input is capped at 1,000,000 bytes and conversion depth is capped so a
browser tab cannot be wedged by a huge schema. Cross-document features such as
`xs:include`, `xs:import`, `xs:redefine`, substitution groups and XSD 1.1
`xs:assert` are reported as named errors instead of guessed, because they require
other files or XPath evaluation.

## FAQ

<details>
<summary>Does this validate XML?</summary>

No. It converts an XSD schema into a JSON Schema document. You still need to map
your XML instances into JSON consistently before validating the JSON data.

</details>

<details>
<summary>How are XML attributes represented?</summary>

Attributes become ordinary JSON properties with the configured prefix. The
default prefix is `@`, so `currency` becomes `@currency`. Set the prefix to an
empty string only when you know attribute names cannot collide with child element
names.

</details>

<details>
<summary>What happens to xs:choice?</summary>

Choice members are emitted as optional properties, and when required derivation is
enabled the converter adds a `oneOf` constraint requiring exactly one of the
choice members. Turn off required derivation if you want a looser structural
schema.

</details>

<details>
<summary>Can it resolve xs:include or xs:import?</summary>

No. This browser-local tool receives one pasted document and does not fetch other
schema files. Include, import, redefine, substitution groups and XSD 1.1 asserts
return explicit errors so the output does not pretend to know missing context.

</details>
