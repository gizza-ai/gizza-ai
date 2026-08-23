## About this tool

**XML Diff** compares two XML documents by **structure**, not by text lines. A
line-based diff flags every re-indentation and every reordered attribute; this
one parses both documents into element trees and reports only the differences
that actually change the data.

Paste your **first (left / old)** document and your **second (right / new)**
document. The tool walks both trees in parallel:

- **Elements** are matched child by child, using the sibling-matching strategy
  you choose.
- **Attributes** are compared as a sorted map, so attribute **order never
  matters**.
- **Text** is whitespace-normalized by default, so indentation and line breaks
  never matter. **CDATA** is folded into the element's text, so
  `<a><![CDATA[hi]]></a>` equals `<a>hi</a>`.
- Every difference is reported with an **XPath-style path** —
  `/catalog/book[2]` for an element, `/catalog/book[2]/@id` for an attribute,
  `/catalog/book[2]/title/text()` for text — and classified as **added**,
  **removed** or **changed**.

### Worked example

Comparing

```xml
<catalog><book id="1"><title>Rust</title></book></catalog>
```

with

```xml
<catalog><book id="2"><title>Rust 2</title></book></catalog>
```

gives this report (JSON format, indent 2):

```json
{
  "equal": false,
  "added": 0,
  "removed": 0,
  "changed": 2,
  "changes": [
    { "path": "/catalog/book/@id", "kind": "changed", "old": "1", "new": "2" },
    { "path": "/catalog/book/title/text()", "kind": "changed", "old": "Rust", "new": "Rust 2" }
  ]
}
```

The same comparison with **Report format = text** gives:

```text
2 differences: 0 added, 0 removed, 2 changed
~ /catalog/book/@id  1 -> 2
~ /catalog/book/title/text()  Rust -> Rust 2
```

### Options

- **Sibling matching** — *Smart alignment* (default) aligns identical subtrees
  first, so inserting one element in the middle is reported as a single
  addition instead of shifting every later sibling. *By position* compares
  siblings strictly index by index. *Ignore order* treats siblings as a set, so
  a reordered document compares as equal.
- **Ignore insignificant whitespace** (on by default) collapses whitespace runs
  and drops whitespace-only text nodes. Turn it off for an exact text
  comparison.
- **Ignore comments** (on by default). Turn it off and comment changes appear
  as `comment()` nodes.
- **Ignore namespace prefixes** makes `ns:book` and `p:book` match and drops
  `xmlns` declarations from the comparison.
- **Compare numbers numerically** makes `1` equal `1.0` and `2.50` equal `2.5`
  in both text and attribute values.
- **JSON indent** sets the output indentation (0 minifies); it is ignored for
  the text report.

Everything runs **locally in your browser** via WebAssembly — your documents are
never uploaded.

### Limits and edge cases

- Each document is limited to **1 MB** and **500 nesting levels**; larger input
  is rejected with a clear error naming the side that failed.
- **Mixed content**: an element's direct text nodes are folded into one value,
  so moving text around between sibling elements inside the same parent is not
  reported positionally.
- The **XML declaration, DOCTYPE and processing instructions** are not
  compared.
- Custom **DTD-defined entities** are compared as written when they cannot be
  resolved.

### Handy for

- Reviewing changes between two config, POM, SOAP or feed versions.
- Checking that a re-serialized document is semantically unchanged.
- Producing a machine-readable change report for tests, CI or audits.

### FAQ

<details>
<summary>Does attribute order or indentation matter?</summary>

No. Attributes are compared as a sorted map, so `<a x="1" y="2"/>` and `<a y="2" x="1"/>` are equal. Indentation and line breaks are ignored too, as long as **Ignore insignificant whitespace** is on (it is by default) — turn it off if you need a byte-faithful text comparison.

</details>

<details>
<summary>How is a reordered document handled?</summary>

By default (*Smart alignment*) sibling order is significant, so swapping two children is reported as differences. Pick **Ignore order — treat siblings as a set** when the order carries no meaning: identical subtrees pair up in any order, then the leftovers pair up by element name.

</details>

<details>
<summary>What happens when an element is inserted in the middle of a list?</summary>

With *Smart alignment* the untouched siblings on both sides of the insertion are matched as anchors, so you get exactly one `added` entry. With *By position* the same edit shifts every later sibling, so you get a string of `changed` entries plus one `added` at the end — useful when position itself is what you're checking.

</details>

<details>
<summary>Are namespaces compared?</summary>

Yes, by default: `ns:book` and `p:book` are different element names even when both prefixes are bound to the same URI, and `xmlns` declarations are ordinary attributes. Switch on **Ignore namespace prefixes** to compare local names only and skip `xmlns` declarations entirely.

</details>

<details>
<summary>Why do I get "the first (left) XML is not well-formed"?</summary>

Both inputs must be well-formed XML, and the error names the side that failed plus the byte position. Common causes: an unclosed tag, a stray `&` that isn't an entity, or mismatched start/end tag names. The tool never repairs input — it only reports.

</details>

<details>
<summary>Is this the same as a text diff of the two files?</summary>

No. A text diff works line by line, so reformatting a document produces a huge diff even when nothing changed. This tool compares the parsed trees, so it reports only real structural and value differences — and it locates each one by element path rather than by line number.

</details>
