## About this tool

Use this XML-to-XSD generator when you have a representative XML instance and need a draft W3C XML Schema to start validation, documentation, or integration work. It parses the sample in the browser, merges repeated elements into one model, infers attributes, child cardinality, text types, namespace handling, `xsi:nil`, mixed content, and repeated or unordered children, then emits an XSD document.

The default Venetian-blind layout creates one global root element plus named complex types. That is the safest default for real XML because it handles recursive structures and keeps the result readable. Salami-slice and Russian-doll layouts are available when you need those design patterns for an existing schema style.

### Worked example

Paste this XML with the default options:

```xml
<order id="7">
  <total>19.95</total>
  <item sku="PEN">pen</item>
  <item sku="PAD">pad</item>
</order>
```

The generated schema declares `order`, a repeated `item`, required `id` and `sku` attributes, `total` as a decimal, and `item` text as a string. Repeated siblings become `maxOccurs="unbounded"`; a child missing from some repeated parents becomes `minOccurs="0"` in restricted mode.

### Design choices

- `venetian-blind`: one global root element and named complex types. Best general-purpose default.
- `salami-slice`: every element is global and parents use `ref` references. Useful for highly reusable element vocabularies.
- `russian-doll`: all declarations are nested under the root. Compact for tiny non-recursive samples, but it cannot express recursive shapes safely.

### Limits and caveats

A single sample cannot prove business rules. It cannot know whether a field is truly optional, whether a code list is complete, or whether a numeric value should be constrained more tightly than its observed type. Treat the output as a draft schema to review. The input cap is 1,000,000 bytes; use a small representative document rather than a production dump. Multi-document merging and full XSD validation are outside this tool; if you need to infer from several samples, combine representative cases under a temporary root and review the result.

## FAQ

<details>
<summary>Is the generated XSD production-ready?</summary>

It is a strong starting point, not a final authority. The tool can infer structure and observed value types, but only you know domain constraints such as allowed product codes, required-in-production fields that are absent from the sample, or versioning rules. Review the draft before using it as a contract.

</details>

<details>
<summary>Why is enumeration inference off by default?</summary>

Enumerations are easy to overfit from one sample. Seeing `new` and `paid` in a file does not prove those are the only valid statuses. Set the enumeration cap only when the sample intentionally includes a complete small code list.

</details>

<details>
<summary>When should I use relaxed occurrence bounds?</summary>

Use relaxed mode when the sample is incomplete or you are exploring an unknown feed. It makes child elements optional and repeatable and keeps attributes optional, so the draft schema accepts a wider range of documents while you collect more examples.

</details>

<details>
<summary>How are XML namespaces handled?</summary>

If the sample root has a default namespace, the generator uses it as the target namespace and emits `tns:` references with `elementFormDefault="qualified"`. You can override the target namespace manually. Foreign-namespace children are represented with an `xs:any` wildcard instead of pretending they belong to the root namespace.

</details>
