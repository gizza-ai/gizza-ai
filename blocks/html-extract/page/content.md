## About this tool

This is "jq for markup": paste raw HTML, write a **CSS selector**, and pull the
same piece out of every element that matches. Pick what to extract — the visible
**text**, the element's **inner HTML** (its children), its **outer HTML** (the
element itself, tags and all), or the value of a named **attribute** such as
`href` or `src`. The result is JSON with a `count` and a `matches` array, so it
drops straight into a script or a spreadsheet.

Everything runs locally in your browser via WebAssembly — the HTML you paste is
never uploaded.

### Worked example

Paste this HTML:

```html
<ul>
  <li><a href="/one">First</a></li>
  <li><a href="/two">Second</a></li>
</ul>
```

With selector `a` and **Extract = Attribute**, attribute name `href`, you get:

```json
{
  "count": 2,
  "matches": [
    "/one",
    "/two"
  ]
}
```

Switch **Extract** to *Text* and the same selector returns `["First", "Second"]`
instead. Use a tighter selector (for example `li:first-child a`) to narrow the
matches, or raise **Max matches** if you have more than 100 results.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>What selectors are supported?</summary>

Standard CSS selectors: tag names (`p`), classes (`.link`), IDs (`#main`),
attribute selectors (`a[href]`), combinators (`div > p`, `ul li`), and
pseudo-classes like `:first-child` and `:nth-of-type(2)`. XPath is **not**
supported — this tool is CSS-only.

</details>

<details>
<summary>What's the difference between inner HTML and outer HTML?</summary>

For `<div><b>hi</b></div>` selected by `div`, **inner HTML** returns
`<b>hi</b>` (just the children) while **outer HTML** returns
`<div><b>hi</b></div>` (the element itself, its own tags included). **Text**
returns just `hi`.

</details>

<details>
<summary>How do I pull an attribute like a link's URL or an image's source?</summary>

Set **Extract** to *Attribute* and type the attribute name — `href` for links,
`src` for images, `class`, `id`, `data-*`, and so on. Elements that match the
selector but don't have that attribute are skipped, so the `count` reflects only
the elements that actually had a value.

</details>

<details>
<summary>Why is the whitespace collapsed in my results?</summary>

**Normalize whitespace** is on by default: it collapses runs of spaces and
newlines in text and attributes to single spaces and trims the ends of HTML.
Turn it off to get the exact original text, indentation and all.

</details>

<details>
<summary>Is my HTML uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs entirely in your browser, so
the HTML you paste never leaves your machine.

</details>
