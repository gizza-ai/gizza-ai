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

## FAQ

<details>
<summary>Can I query a real-world HTML page with this?</summary>

Only if it's well-formed. The evaluator parses strict XML/XHTML — it is not a
forgiving HTML parser, so typical scraped HTML (unclosed `<li>`, bare `<br>`,
unquoted attributes) fails to parse. Run the page through an HTML tidier, or
use the html-to-markdown tool if you just want the content.

</details>

<details>
<summary>What's the difference between the "value" and "xml" output modes?</summary>

`value` (the default) prints each matched node's string value — the
concatenated text content, which for an attribute is just its value. `xml`
serializes the whole matching node as outer XML, including its tag and
attributes, with text properly re-escaped; an empty element comes out
self-closing.

</details>

<details>
<summary>Which XPath version and functions are supported?</summary>

XPath **1.0** — node-set paths with predicates, plus the 1.0 function library
(`count()`, `name()`, `string()`, comparisons returning booleans, and so on).
Scalar expressions return a single number, string, or boolean; XPath 2.0+
features such as `matches()` or sequences aren't available.

</details>

<details>
<summary>Why does my query return nothing on a namespaced document?</summary>

In XPath 1.0, an unprefixed name only matches elements in **no** namespace —
so `//svg` won't match `<svg xmlns="http://www.w3.org/2000/svg">`. The page
UI doesn't currently let you register namespace prefixes; a common workaround
is matching by local name, e.g. `//*[local-name()='svg']`.

</details>
