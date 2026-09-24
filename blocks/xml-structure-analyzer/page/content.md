## About this tool

Use this XML structure analyzer when you need to understand an unfamiliar XML document before writing an XPath, mapping it to CSV, or debugging a feed. It parses the document locally and reports the root element, a collapsed element tree, per-tag counts, maximum and average depth, a depth histogram, attribute usage, namespaces, XML declaration and DOCTYPE information, node-type counts, and structural warnings.

Worked example:

```xml
<catalog>
  <book id="b1"><title>Dune</title><price currency="USD">9.99</price></book>
  <book id="b2"><title>Emma</title><price currency="EUR">7.50</price></book>
  <meta/>
</catalog>
```

The report shows `catalog` as the root, `book` as a repeated child, `title` and `price` below each book, a maximum depth of 3, two `id` attributes on `book`, two `currency` attributes on `price`, and one empty `meta` element.

## Limits and edge cases

- The tree is collapsed by element path, so repeated siblings such as many `<item>` entries appear once with a count.
- Namespace prefixes are kept in tag names as written, and `xmlns` declarations are reported separately from ordinary attributes.
- Whitespace-only text is ignored; non-empty text, CDATA, comments, and processing instructions are counted.
- `tree_depth = 0` renders the full collapsed tree. Set a positive value to cap deep documents.
- This is a structure report, not an XML formatter, XML-to-JSON converter, or XSD validator.

## FAQ

<details>
<summary>Does this validate XML schemas?</summary>

No. It checks well-formed XML while parsing and reports line and column information for parse errors, but it does not validate against XSD or DTD schemas.

</details>

<details>
<summary>Why do repeated XML nodes appear only once in the tree?</summary>

The tree collapses repeated siblings by element path to keep large feeds readable. For example, one thousand `<item>` nodes under `<channel>` render as `channel/item (1000)` instead of one thousand separate lines.

</details>

<details>
<summary>How are attributes counted?</summary>

Ordinary attributes are counted globally and per element name. Namespace declarations such as `xmlns` and `xmlns:soap` are reported in the Namespaces section instead of being mixed into the attribute table.

</details>

<details>
<summary>When should I use CSV output?</summary>

Choose CSV when you want a spreadsheet-friendly tag inventory. It emits one row per distinct element tag with count, depth range, child count, text count, empty count, and optional attribute summaries.

</details>
