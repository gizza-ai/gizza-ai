## What this tool does

Paste an indented text outline and get back nested JSON — instantly, right in
your browser. Nothing is uploaded to a server: it runs locally, works offline,
and needs no sign-up. Each non-blank line becomes a node, and the **leading
whitespace** of a line decides where it sits in the tree.

## How nesting works

A line that is **more indented** than the line before it becomes a **child** of
the nearest preceding less-indented line. The same indentation makes lines
**siblings**, and a line that dedents reattaches to the matching ancestor. Both
**spaces and tabs** work, and you can even mix them — set the **Tab width** to
say how many columns a tab counts for (default 4). The depth steps don't have to
be uniform: only the relative indentation matters, so a 2-space outline and a
4-space outline both parse correctly.

Blank lines (and whitespace-only lines) are ignored, so you can space your
outline out for readability. The **first line must not be indented** — it's the
root.

## Output shapes

| Shape | Output | Good for |
| --- | --- | --- |
| **children** (default) | An array of `{ "text": …, "children": […] }` nodes. Order and duplicate siblings are preserved; a leaf has an empty `children` array. | Faithful tree data, lists with repeated labels, rendering a UI tree. |
| **nested** | An object keyed by each node's text, like `{ "a": { "b": {} } }`. A leaf is an empty object; duplicate sibling keys keep the last. | Compact config-style trees, quick lookups by name. |

Turn **Pretty-print** off for compact, single-line JSON.

## Examples

Outline:

```
Project
  Frontend
    UI
  Backend
    API
```

**children** output (pretty):

```json
[
  {
    "text": "Project",
    "children": [
      { "text": "Frontend", "children": [ { "text": "UI", "children": [] } ] },
      { "text": "Backend", "children": [ { "text": "API", "children": [] } ] }
    ]
  }
]
```

**nested** output:

```json
{ "Project": { "Frontend": { "UI": {} }, "Backend": { "API": {} } } }
```

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your outline never leaves your device, and the
page keeps working offline once it has loaded.

</details>

<details>
<summary>Can I mix tabs and spaces?</summary>

Yes. A tab counts as **Tab width** columns
(default 4), so a tab-indented child still nests correctly under a
space-indented parent.

</details>

<details>
<summary>What if my indentation isn't a consistent number of spaces?</summary>

That's fine —
only the *relative* indentation between lines matters. A line that is more
indented than its predecessor becomes a child no matter the exact step size.

</details>

<details>
<summary>Which output shape should I pick?</summary>

Use **children** when order or repeated
labels matter, or when you want explicit `text`/`children` fields. Use
**nested** for a compact object tree you can look up by name.

</details>

<details>
<summary>Why did I get an error?</summary>

The first line must not be indented (it's the root),
and the outline must contain at least one non-blank line.

</details>
