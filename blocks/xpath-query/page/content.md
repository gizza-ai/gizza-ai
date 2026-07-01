## About this tool

**XPath query** evaluates an XPath 1.0 expression against an XML or XHTML
document and returns the matching values. Use it to extract element text,
attribute values, filtered node sets, counts, booleans, and function results.

- **Node-set queries:** `//book/title`, `//a/@href`, or `//item[price < 10]`
  return one line per match in document order.
- **Scalar expressions:** `count(//book)`, `name(/*)`, or `//price > 100`
  return a single number, string, or boolean.
- **Output mode:** choose `value` for each node's string value, or `xml` to
  serialize matching nodes as outer XML.

### Examples

- `//book/title` extracts all book titles.
- `//a/@href` extracts link targets from XHTML.
- `count(//item)` counts matching elements.
- `//book[price < 10]/title` filters with a predicate.

### Scope and limitations

This is an XPath 1.0 evaluator for well-formed XML/XHTML. It is not a forgiving
HTML parser; tidy malformed HTML before querying it. Namespace-heavy documents
may need namespace-aware expressions that the current simple page UI does not
configure separately.

### Privacy

The browser version runs locally in WebAssembly, so your XML document and XPath
expression stay on your device.
