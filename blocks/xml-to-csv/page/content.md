## About this tool

XML to CSV turns a block of XML with repeated elements into a flat CSV table you
can open in a spreadsheet, load into a database, or hand to another tool. It runs
entirely in your browser — your data is never uploaded.

Each repeated **record** element becomes one row. The tool reads the record's
child elements and uses their tags as the CSV column headers, in the order they
first appear. Nested elements are flattened into dot-notation columns — an
`<address><city>` becomes the column `address.city` — so deeply structured XML
maps cleanly onto a table without losing data. Missing fields become empty cells,
repeated tags are kept as `tag`, `tag.2`, `tag.3`, and XML entities (`&amp;`,
`&lt;`) and `CDATA` blocks are decoded back to plain text. Values that contain the
delimiter, quotes, or newlines are CSV-quoted automatically.

## How to use it

1. Paste your XML.
2. Leave **Record tag** blank to auto-detect the repeated element (the most
   common direct child of the root), or type a tag such as `book` or `item`.
3. Toggle **Include attributes as columns** to add attribute values — `@id` on
   the record element, `price@currency` on a child element.
4. Pick a delimiter if you need tab, semicolon, or pipe instead of a comma.

## Example

Input:

```xml
<users>
  <user id="1"><name>Ada</name><role>admin</role></user>
  <user id="2"><name>Bo</name><role>editor</role></user>
</users>
```

With attributes on, record tag `user`, you get:

```csv
@id,name,role
1,Ada,admin
2,Bo,editor
```

## Notes

- Namespaced tags (`ns:title`) are flattened to their local name (`title`).
- Auto-detect picks the most frequent direct child of the root element; set the
  record tag explicitly if your XML nests records deeper.
- Everything is computed locally and deterministically — no account, no AI model,
  no server round-trip.
