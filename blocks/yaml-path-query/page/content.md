## Query and edit YAML by path in your browser

Paste a YAML document, enter a path such as `server.host`, `items[0].name` or
`['key with spaces']`, and choose whether to query, set or delete that value.
The tool runs locally in your browser and uses the same pure-Rust core as the
CLI and chat block.

### Worked example

Input YAML:

```yaml
server:
  host: localhost   # bind address
  port: 8080
items:
  - name: alpha
    qty: 1
```

- Path `server.host` in **Query** mode returns `localhost`.
- Path `server.port` in **Set** mode with value `9090` returns the whole YAML
  document with only that scalar changed; the trailing comment on `host` stays
  where it was.
- Path `items[0].name` reads `alpha`; `items.0.name` is accepted too.

### Path syntax

- Dot segments select mapping keys: `metadata.name`.
- Bracketed numbers select list items: `containers[0].image`.
- Plain numeric dot segments also select list items: `containers.0.image`.
- Quoted bracket keys select keys that contain punctuation or spaces:
  `['my.key']`, `["key with spaces"]`.
- A leading `$` root marker is accepted (`$.server.host`) for copy-pasted paths,
  but JSONPath filters, wildcards and recursive descent are intentionally not
  part of this small editor.

### Modes and output

- **Query** returns scalar values raw, which makes the result easy to copy or
  pipe. Mapping and list results are printed as YAML.
- **Set** parses the value as YAML: `42`, `true`, `null`, `[a, b]` and `{k: v}`
  become typed YAML values. Wrap a value in quotes, for example `"8080"`, to
  force a string.
- **Delete** removes a mapping key or list item.
- **JSON output** pretty-prints the selected value or edited document as JSON.

### Limits and edge cases

The input must contain one YAML document. Multi-document streams separated by
`---` are rejected so the target document is never guessed. Simple scalar edits,
new keys in an existing block mapping and block item deletes are spliced into the
original source and then re-parsed to verify the result, preserving comments,
blank lines, key order, quoting style and indentation. More complex edits, such
as creating missing intermediate levels or replacing block scalars, fall back to
re-emitting the parsed tree; that keeps the data correct but normalizes
formatting and drops comments. YAML anchors resolve during query, but anchor
syntax is not preserved when an edit needs the normalized fallback.

## FAQ

<details>
<summary>Is this JSONPath for YAML?</summary>

No. It accepts familiar dot and bracket paths, but it is not RFC 9535 JSONPath.
There are no wildcards, filters, slices or recursive descent operators. The goal
is a predictable single-node path for quick YAML reads and edits.

</details>

<details>
<summary>Will comments and formatting survive edits?</summary>

Often, yes. Existing scalar replacements, simple new keys and block deletes are
applied as verified text splices. If a safe splice is not possible, the tool
falls back to re-emitting the YAML from the parsed data tree, which is correct
but may drop comments and normalize formatting.

</details>

<details>
<summary>How do I address a key that contains a dot?</summary>

Use a quoted bracket segment. For example, if the YAML has a key named
`my.key`, use `["my.key"]` rather than `my.key`, because the latter means the
key `my` followed by the key `key`.

</details>

<details>
<summary>Why did setting a value change its type?</summary>

Set values are parsed as YAML on purpose. `8080` becomes a number, `true` a
boolean and `[a, b]` a list. To force a string, quote the value in the value
field, for example `"8080"`.

</details>
