## What this tool does

Turn a block of **YAML** into a **CSV** table you can open in a spreadsheet.
Paste a top-level **list of records**, or a **mapping** whose values are records,
and every record becomes one row. The header is the **union of every record's
keys**, in the order they are first seen, so rows that are missing a field simply
leave that cell blank instead of shifting columns. Nested mappings are flattened
to **dot-path** columns (`address.city`), and arrays are rendered the way you
choose. Everything runs in your browser — the YAML never leaves your device.

## How the shape is read

- **A list of records** — the common case. Each list item is a row:

  ```yaml
  - name: Ada
    age: 36
    address:
      city: London
  - name: Bo
    age: 40
  ```

  becomes `name,address.city,age` with one row per person.

- **A mapping of records** — when the top level is a mapping whose values are
  themselves records, each entry is a row and its key is kept in an extra column
  (named by **Mapping key column name**, `key` by default):

  ```yaml
  alice:
    age: 30
  bob:
    age: 40
  ```

  becomes `key,age` / `alice,30` / `bob,40`.

- **A single record** — a top-level mapping whose values are all scalars is
  treated as one row (no key column).

## Options

- **Delimiter** — separate columns with a comma, semicolon, tab, or pipe.
- **Include header row** — turn off to emit data rows only.
- **Array handling** — `json` writes an array as one compact JSON string in a
  single cell (`["a","b"]`); `joined` joins a scalar array with `, ` in one cell;
  `columns` expands each element into its own dot-indexed column (`tags.0`,
  `tags.1`).
- **Quote every field** — wrap all cells in double quotes, not just the ones that
  contain a delimiter, quote, or newline.
- **Mapping key column name** — the header for the key column when the input is a
  mapping of records; leave it blank to drop the key.

## Limits

- The whole document is parsed in memory, so extremely large files (tens of MB)
  may be slow in a browser tab.
- Only the **top level** decides the shape: a list of records, a mapping of
  records, or a single scalar mapping. A top-level scalar, or a top-level list of
  scalars, has no columns and is reported as an unsupported shape.
- Deeply nested mappings flatten to dot-paths; if two different structures
  produce the same dot-path, the later value wins.
- YAML anchors/aliases are expanded by the parser; custom `!tags` are dropped and
  only their value is kept.

## FAQ

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The conversion runs entirely in your browser with WebAssembly, so the YAML
you paste never leaves your device and the tool keeps working offline once the
page has loaded.

</details>

<details>
<summary>What happens when records don't all have the same keys?</summary>

The header is the **union** of every record's keys, in first-seen order. A record
that is missing a key just gets an empty cell for that column, so columns never
shift out of alignment.

</details>

<details>
<summary>How are nested objects and arrays handled?</summary>

Nested mappings are flattened to dot-path columns — `address: {city: London}`
becomes a column named `address.city`. Arrays follow the **Array handling**
option: kept as a compact JSON string, joined into one cell, or expanded into
`name.0`, `name.1` columns.

</details>

<details>
<summary>Which delimiters and quoting rules are used?</summary>

Pick comma, semicolon, tab, or pipe as the delimiter. Fields are quoted only when
they contain the delimiter, a double quote, or a newline (embedded quotes are
doubled), following the usual CSV rules — or turn on **Quote every field** to
quote everything.

</details>

<details>
<summary>Why do I get an "unsupported top-level YAML shape" error?</summary>

The converter needs records to build columns from. A top-level list of records or
a mapping of records works; a bare scalar (`just a string`) or a list of plain
scalars has no fields to become columns, so it is rejected with a clear message.

</details>
