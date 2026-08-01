## About this tool

JSON to Markdown renders an arbitrary JSON document — a notes-app export, an API response, a config file — as clean, nested Markdown. A top-level object's scalar keys become `- **key**: value` bullets and its nested objects and arrays become headings, starting at your chosen heading level and going one level deeper per nesting. A uniform array of flat objects becomes a GitHub-flavored table; other arrays become nested bullet lists. Everything runs in your browser — nothing is uploaded.

Worked example — a note object:

Input:

```json
{"title":"My Note","pinned":true,"tags":["work","idea"],"meta":{"words":42,"created":"2026-07-31"}}
```

Markdown output (default settings):

```markdown
- **title**: My Note
- **pinned**: true

## tags

- work
- idea

## meta

- **words**: 42
- **created**: 2026-07-31
```

Worked example — records become a table:

Input:

```json
[{"name":"Ada","role":"Editor"},{"name":"Bob","role":"Reviewer"}]
```

Markdown output:

```markdown
| name | role |
| --- | --- |
| Ada | Editor |
| Bob | Reviewer |
```

Set **Array rendering** to `list` to force nested bullets instead of a table, or `table` to force a table even for ragged or nested arrays (non-scalar cells become inline JSON).

## Limits and edge cases

- Input must be valid JSON; a parse error is reported rather than a partial render.
- **Starting heading level** is 1–6 (default 2); each nesting level goes one deeper, capped at `######`.
- **Max nesting depth** is 1–20 (default 6); any subtree deeper than this is emitted verbatim as a fenced `json` code block so very deep data stays bounded and readable.
- A table is used only for a uniform array of flat (scalar-valued) objects under `auto`; its columns are the union of all row keys, and missing cells are left blank.
- Table cells are collapsed to a single line and pipe characters are escaped as `\|`; `null` renders as an empty value.
- **Sort keys** orders object keys and table columns alphabetically; by default the document's original key order is preserved.
- An empty object renders as `_(empty object)_` and an empty array as `_(empty array)_`.

## FAQ

<details>
<summary>How does an array of objects turn into a table?</summary>

Under the default `auto` mode, a uniform array whose objects hold only scalar values becomes a GitHub-flavored pipe table. The columns are the union of every object's keys, so rows with missing keys get blank cells. If any object contains a nested object or array, the whole array falls back to a nested bullet list instead — or you can set **Array rendering** to `table` to force a table anyway, in which case nested cells are rendered as inline JSON.

</details>

<details>
<summary>What happens to deeply nested JSON?</summary>

Nesting is expanded as headings and bullets up to **Max nesting depth** (default 6). Any subtree deeper than that limit is emitted verbatim inside a fenced `json` code block rather than being flattened further. Lower the limit to keep large documents compact, or raise it (up to 20) to expand everything.

</details>

<details>
<summary>Does it keep my key order, or sort them?</summary>

By default keys and table columns appear in the document's original order. Enable **Sort keys alphabetically** to order every object's keys and every table's columns alphabetically instead.

</details>

<details>
<summary>Is my JSON uploaded anywhere?</summary>

No. The conversion runs entirely in your browser through WebAssembly — the JSON you paste never leaves your machine, so it is safe for private notes, config, or API payloads.

</details>
