## What this tool does

Paste **messy, inconsistently-indented, or minified** YAML and get it back
cleanly **reindented and normalized**. Choose how many spaces to indent per
nesting level, alphabetize every mapping's keys, or collapse the whole document
to compact **flow** style. It parses the YAML, so invalid input is reported as
an error instead of being silently mangled.

Everything runs locally in your browser via WebAssembly — your YAML never leaves
your machine and there are no network requests.

## Options

- **Spaces per level** — indentation width for block style, from 1 to 8 (default
  2). YAML forbids tab characters for indentation, so only spaces are offered.
- **Output style**
  - **block** — expanded, multi-line YAML (the usual, readable layout).
  - **flow** — compact, single-line JSON-like YAML (`{name: gizza, tags: [a, b]}`),
    handy for minifying or embedding a value on one line.
- **Key order**
  - **preserve** — keep the original key order (default).
  - **asc** / **desc** — sort every mapping's keys alphabetically, recursively —
    useful for producing diff-stable config files.

## Worked example

Input (inconsistent spacing and inline collections):

```yaml
name:   gizza
tags: [a, b]
nested: {x: 1}
```

Style `block` · Indent `2` spaces · Key order `preserve`

Output:

```yaml
name: gizza
tags:
  - a
  - b
nested:
  x: 1
```

Switch **Output style** to `flow` to get `{name: gizza, tags: [a, b], nested: {x: 1}}`
on a single line, raise **Spaces per level** to `4` to widen the indentation, or
set **Key order** to `asc` to alphabetize the keys.

## Limits and edge cases

- This is a **formatter**, not a linter or schema validator — it checks that the
  YAML *parses* and re-emits it, but it doesn't check your data against a schema.
- **Comments, blank lines, and anchors are not preserved.** The document is parsed
  into a data model and re-emitted, so `# comments`, spacing between blocks, and
  `&anchor`/`*alias` references are dropped — **aliases are expanded** to the value
  they point at.
- Strings that would otherwise be read back as a boolean, null, or number
  (`"true"`, `"null"`, `"123"`) are **double-quoted** to keep their type. Other
  strings are left unquoted where that is unambiguous.
- Indentation is clamped to 1–8 spaces; tabs are not used (YAML forbids tab
  indentation).
- Multi-document streams separated by `---` are supported; each document is
  normalized independently.
- To convert YAML to or from JSON/TOML instead of reformatting it, use a
  dedicated converter — this tool always outputs YAML.

## FAQ

<details>
<summary>Does formatting change my data?</summary>

The **values** are preserved — the document is parsed and re-emitted with the
same data. What changes is the *presentation*: indentation, quoting, key order
(if you sort), and block-vs-flow layout. Comments, blank lines, and anchors are
not carried over, and aliases are expanded to their referenced value.

</details>

<details>
<summary>Can I indent with tabs?</summary>

No. The YAML specification forbids tab characters for indentation, so the tool
only offers spaces (1 to 8 per level). If your editor inserts tabs, this
formatter will replace them with consistent spaces.

</details>

<details>
<summary>What is the difference between block and flow style?</summary>

**Block** style is the familiar expanded, multi-line layout where each key and
list item sits on its own indented line. **Flow** style is compact and
JSON-like, putting mappings in `{ }` and sequences in `[ ]` on a single line —
useful for minifying a document or writing a short value inline.

</details>

<details>
<summary>Why were some of my strings put in quotes?</summary>

A plain YAML scalar like `true`, `null`, or `123` is read back as a boolean,
null, or number. When your data holds those as **strings**, the formatter
double-quotes them (`"true"`, `"123"`) so they keep their string type on the next
parse. Values that are unambiguous are left unquoted.

</details>

<details>
<summary>Why did my YAML fail to format?</summary>

The formatter parses the input first, so anything that isn't valid YAML — an
unterminated flow collection, bad indentation, or a duplicate key — is reported
as an error with the parser's message rather than being guessed at. Fix the
reported issue and try again.

</details>

<details>
<summary>Is my YAML uploaded anywhere?</summary>

No. The formatter runs entirely in your browser via WebAssembly. Your document
never leaves your machine and there are no network requests.

</details>
