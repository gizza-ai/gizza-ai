## About this tool

Convert a JSON document into well-formed XML without uploading data. Objects become nested elements, arrays use a configurable item element, scalar values become text, and `null` values become empty self-closing elements. Keys are sanitized into valid XML names so data with spaces or punctuation still produces parseable XML.

The default mapping treats object keys that start with `@` as attributes and `#text` as element text. For example, `{"book":{"@id":"dune","#text":"Dune"}}` becomes `<book id="dune">Dune</book>`. Change those conventions if you need to match another JSON/XML pipeline, or clear the attribute prefix to render every key as a child element.

Worked example:

Input JSON:

```json
{"book":{"@id":"dune","title":"Dune","authors":["Frank Herbert"]}}
```

With `root_element=catalog` and `array_item_element=author`, the XML is:

```xml
<catalog>
  <book id="dune">
    <title>Dune</title>
    <authors>
      <author>Frank Herbert</author>
    </authors>
  </book>
</catalog>
```

Limits and edge cases: this is a structural converter, not an XML schema validator. JSON has no native distinction between XML attributes, text nodes, comments, namespaces, processing instructions, and repeated sibling names, so those are controlled by simple naming conventions. Very large JSON documents can produce much larger XML; use compact mode when whitespace matters.

## FAQ

<details>
<summary>How are JSON arrays converted?</summary>

Each array value becomes a repeated child element using the array item element name. The default is `item`, so `["a","b"]` under a `tags` key becomes `<tags><item>a</item><item>b</item></tags>`. Change the item name to match your target schema, such as `row`, `entry`, or `author`.

</details>

<details>
<summary>Can I create XML attributes from JSON?</summary>

Yes. By default, scalar object keys starting with `@` become attributes on the parent element. For example, `{"user":{"@id":"42","name":"Ada"}}` renders `<user id="42"><name>Ada</name></user>`. Set a different attribute prefix if another convention is required, or set it empty to disable attribute handling.

</details>

<details>
<summary>What happens to invalid XML tag names?</summary>

Keys are sanitized instead of failing the whole conversion: unsupported characters become underscores, and names that start with a digit get a leading underscore. For example, `"1 bad!"` becomes `<_1_bad_>`. If you need exact tag names, rename those keys in the JSON before converting.

</details>

<details>
<summary>Does this preserve comments, namespaces, or an XML schema?</summary>

No. Plain JSON cannot represent XML comments, namespace declarations, processing instructions, or schema types without a custom convention. This tool focuses on deterministic JSON-to-element conversion with optional attributes and text content; validate the result against your schema afterward if one is required.

</details>
