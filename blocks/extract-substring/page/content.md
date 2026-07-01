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

## FAQ

<details>
<summary>What happens if my start or end index is out of range?</summary>

Nothing breaks — indices are **clamped** to the text length. `end=999` on an
11-character string just means "to the end", and if the resolved start ends up
at or past the end you get an empty result rather than an error.

</details>

<details>
<summary>How do negative indices work?</summary>

Python-style: they count back from the end. On `hello world`, `start=-5` gives
`world`, and `start=0, end=-6` gives `hello`. Leave the end blank to take
everything from the start index to the end of the string.

</details>

<details>
<summary>Are the delimiters regular expressions?</summary>

No — they're matched as **literal text**, so `[`, `</b>` or `"` need no
escaping. The tool returns **every** non-overlapping chunk between the pair,
one per line, scanning left to right; if the pair never matches you get
"No matches between the delimiters." Both delimiters are required in this mode.

</details>

<details>
<summary>Does the index count bytes or characters?</summary>

Characters (Unicode code points), never bytes — so `é`, `漢` or `👍` each count
as one and slicing can't cut a character in half. Only multi-code-point
sequences like family emoji or flag emoji occupy more than one index position.

</details>
