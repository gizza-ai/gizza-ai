## About this tool

**Extract Substring** pulls a portion out of a piece of text, two ways:

- **Index mode** — give a **start** and (optional) **end** character position.
  Positions are 0-based, the end is exclusive, and **negative numbers count from
  the end** (Python-style). Leave the end blank to go to the end of the string.
  Indexing is by *character*, so it's safe with accents and emoji.
- **Delimiters mode** — give a **start delimiter** and an **end delimiter**, and
  get back **every** chunk found between them (e.g. everything inside `[...]`, or
  between `<b>` and `</b>`).

Pick the mode from the dropdown and fill in the fields it uses. Everything runs
**locally in your browser** via WebAssembly — your text is never uploaded.

### Examples

- Index: `start=0, end=5` on "hello world" → `hello`; `start=-5` → `world`.
- Delimiters: `[` / `]` on "a[1]b[2]c[3]" → `1`, `2`, `3`.
