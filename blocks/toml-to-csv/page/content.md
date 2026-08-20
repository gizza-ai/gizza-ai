## Convert a TOML array-of-tables into a spreadsheet

TOML is a config format, so its repeated `[[section]]` blocks are really a table in disguise: every `[[users]]` entry is a record and every key inside it is a field. This tool turns that structure into CSV — one row per entry, one column per key — so you can open a `Cargo.toml`, a `pyproject.toml`, a Netlify or Hugo config, or any hand-written TOML data file in a spreadsheet, load it into a database, or diff it against another export.

The header is the **union of every key seen across all entries**, not just the keys of the first one. Config files are routinely ragged — one server has a `region`, the next doesn't — and taking the first entry as the schema would silently drop the rest. Missing values become empty cells instead.

Everything runs in your browser through WebAssembly. The TOML you paste is never uploaded.

### Worked example

Input:

```toml
[[users]]
name = "Ada"
role = "admin"
tags = ["founder", "ops"]

[[users]]
name = "Linus"
role = "dev"
```

With the defaults (auto-detect table, flatten nested tables, arrays as JSON, union header, comma delimiter, header row on):

```csv
name,role,tags
Ada,admin,"[""founder"",""ops""]"
Linus,dev,
```

Switch **Array values** to `Indexed columns` and the same input becomes `name,role,tags.1,tags.2` with `founder` and `ops` in their own cells — ready to sort and filter.

### Nested tables

A sub-table inside an entry has no natural cell to live in, so you choose:

```toml
[[servers]]
name = "web-1"
addr = { city = "Berlin", zip = "10115" }
```

- **Flatten to dot-notation columns** (default) → columns `name`, `addr.city`, `addr.zip`. Best for spreadsheets, because every leaf is filterable.
- **Keep as JSON in one cell** → columns `name`, `addr`, with `{"city":"Berlin","zip":"10115"}` in the cell. Best when a downstream tool re-parses the value.
- **Skip nested tables** → column `name` only. Best when the nesting is metadata you don't want in the export.

Flattening recurses, so `[servers.addr.geo]` becomes `addr.geo.lat`.

### Controls and limits

- **Array-of-tables path**: leave blank to auto-detect — the first root-level array-of-tables wins, then nested ones. Give a dotted path like `servers.pool` to pick a specific one when a document has several. Quoted TOML keys (`["odd key"]`) can't be addressed this way.
- **Header columns**: `union` keeps first-seen document order, `sorted` is alphabetical (stable across files with different key orders), `first` locks the schema to the first entry and drops later extras.
- **Array values**: `json`, `join` (`a; b` in one cell), or `columns` (1-based `tags.1`, `tags.2` — the column count is driven by the longest array in the document).
- **CSV delimiter**: comma, semicolon, tab, or pipe. Output is RFC 4180 quoted, so a value containing the delimiter, a `"`, or a newline is quoted and escaped correctly.
- **Include header row**: turn off to append rows to a sheet that already has headers.
- Capped at 20,000 rows and 2,000 columns per run. For larger files, use the CLI command shown above in a local pipeline.
- **Comments are not preserved.** TOML parsers discard them, so `# note` lines never reach the CSV.
- A document with no array-of-tables at all is emitted as a single row of its top-level keys, which is a convenient way to flatten a plain config file.

## FAQ

<details>

<summary>What is an "array of tables" and how do I know if my file has one?</summary>

It is the repeated double-bracket form: `[[users]]` written two or more times, each block holding the same kind of keys. That is TOML's way of writing a list of records, and it is the shape that maps cleanly to CSV rows. A single `[users]` (one bracket) is a plain table and converts to a single row instead.

</details>

<details>

<summary>My file has several `[[...]]` sections. Which one gets converted?</summary>

Leave the path blank and the first root-level array-of-tables in document order wins; if there is none at the root, the first nested one is used. To pick a different one, type its dotted path — `servers.pool` for `[[servers.pool]]`. If the path doesn't exist, the error message lists every array-of-tables the tool did find, so you can copy the right one.

</details>

<details>

<summary>Why are some cells empty?</summary>

Because the header is the union of keys across all entries. If one `[[users]]` block has a `region` key and another doesn't, `region` still becomes a column and the entry that lacks it gets an empty cell. That is deliberate — the alternative is losing data. Use **Header columns → First row's keys only** if you want the first entry to define a strict schema instead.

</details>

<details>

<summary>Can I convert CSV back to TOML with this?</summary>

No, this direction is one-way. It is a reshaper, not a round-trip serializer: comments are gone, and the flattening choices (dot columns, joined arrays) aren't reversible without knowing the original types.

</details>

<details>

<summary>What happens to dates, numbers, and booleans?</summary>

They keep their TOML text form. `1979-05-27T07:32:00Z` stays exactly that, `true` stays `true`, and `1.0` stays `1.0` rather than collapsing to `1`. No arithmetic, rounding, locale formatting, or type coercion is applied, so a spreadsheet import sees the same characters that were in the file.

</details>

<details>

<summary>Is my configuration file uploaded anywhere?</summary>

No. The converter is compiled to WebAssembly and runs entirely in your browser tab — the TOML never leaves your device, and the page keeps working offline once loaded. That matters here because config files often contain hostnames, internal service names, and other details you don't want pasted into a remote service.

</details>
