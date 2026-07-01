## What this tool does

Paste a list of file paths or an indented outline and get back a clean **ASCII
directory tree** — the kind you see at the top of a good README. It runs entirely
in your browser: nothing is uploaded, it works offline, and there's no sign-up.

## Two ways to describe your tree

| Mode | You paste | How it nests |
| --- | --- | --- |
| **paths** (default) | One **slash-separated path** per line, like `src/main.rs`. | Lines sharing a leading path are **merged** into one tree. A trailing `/` marks an explicit directory. |
| **outline** | One name per line, indented. | A **more-indented** line becomes a **child** of the line above. Tabs and spaces both work (a tab counts as 4 columns). |

Blank lines are ignored in both modes, so you can space your input out for
readability.

## Options

- **Root label** — the text on the very top line of the tree (default `.`).
- **Plain-ASCII connectors** — swap the Unicode box-drawing characters
  (`├──`, `└──`, `│`) for plain ASCII (`|--`, `` `-- ``, `|`) when you need the
  tree to render in a strictly-ASCII context.
- **Trailing slash on directories** — append `/` to folder names (on by default).
- **Sort** — order each folder's entries with **directories first**, then
  alphabetically. Off by default, which keeps your input order.

## Example

**paths** input:

```
src/main.rs
src/lib.rs
README.md
docs/guide.md
```

Output:

```
.
├── src/
│   ├── main.rs
│   └── lib.rs
├── README.md
└── docs/
    └── guide.md
```

The same tree in **outline** mode:

```
src/
  main.rs
  lib.rs
README.md
docs/
  guide.md
```

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your input never leaves your device, and the
page keeps working offline once it has loaded.

</details>

<details>
<summary>What's the difference between the two modes?</summary>

Use **paths** when you have a
flat list of full paths (e.g. from `git ls-files` or a `find` command); the tool
merges shared folders for you. Use **outline** when you'd rather sketch the
structure by hand with indentation.

</details>

<details>
<summary>Can I mix tabs and spaces in outline mode?</summary>

Yes. A tab counts as 4 columns, so
a tab-indented line still nests correctly under a space-indented parent.

</details>

<details>
<summary>How do I mark an empty directory?</summary>

Give it a trailing slash (`build/`). In
paths mode any path segment before the last is treated as a directory
automatically.

</details>

<details>
<summary>Why ASCII instead of Unicode?</summary>

Some terminals, fonts, or documentation
pipelines don't render box-drawing characters cleanly — plain-ASCII connectors
guarantee the tree looks right everywhere.

</details>
