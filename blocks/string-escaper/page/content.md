## About this tool

**String Escaper** makes a string safe to drop into a chosen syntax — and, where
the escaping is reversible, turns it back into raw text.

Pick a **target**:

- **JSON** — escapes quotes, backslashes, newlines, tabs and control characters
  (`\uXXXX`) for a JSON string body. Tick **Wrap in quotes** to add the `"…"`.
- **JavaScript** — a JS string literal, also escaping `'`, `` ` `` and the
  line-separator code points U+2028 / U+2029 that break JS strings.
- **HTML** — replaces `&`, `<`, `>`, `"` and `'` with HTML/XML entities.
- **URL** — percent-encodes a component (everything outside `A–Z a–z 0–9 - _ . ~`).
- **Shell** — wraps the text as a single safe POSIX single-quoted argument.
- **SQL** — doubles single quotes for a SQL string literal.
- **Regex** — backslash-escapes regex metacharacters so the text matches literally.

**Unescape** reverses the JSON, JavaScript, HTML and URL escapings. Shell, SQL
and regex escaping are one-way (un-escaping them is ambiguous).

Everything runs **locally in your browser** via WebAssembly — nothing is uploaded.

### Handy for

- Pasting a snippet safely into a JSON or HTML config or an API payload.
- Building a shell command or SQL query with untrusted text.
- Turning a fixed string into a literal regex pattern.

## FAQ

<details>
<summary>Why is unescape unavailable for shell, SQL and regex?</summary>

Those escapings aren't uniquely reversible: given `''` in SQL you can't tell
whether the original was a quote or two adjacent strings, and a shell-quoted
or regex-escaped string has several raw forms that produce identical output.
Unescape therefore works only for **JSON, JavaScript, HTML and URL**, where a
strict inverse exists — the other three return an explicit error.

</details>

<details>
<summary>What does the "Wrap in quotes" checkbox actually change?</summary>

It adds the outer delimiters for targets that have them: `"…"` for JSON and
JavaScript, `'…'` for SQL. HTML, URL and regex have no quoting concept, and
the shell target *always* produces one safe POSIX single-quoted argument
regardless of the checkbox.

</details>

<details>
<summary>Should I URL-escape a whole address or just a piece of it?</summary>

Just the piece. The URL target does strict **component** encoding — every
byte outside `A–Z a–z 0–9 - _ . ~` is percent-encoded, including `/`, `:`
and `?`. Escape a query value or path segment and splice it into the URL;
escaping a complete URL would mangle its structure.

</details>

<details>
<summary>Is SQL escaping here enough to stop injection?</summary>

It applies the standard rule — doubling single quotes inside a string
literal — which is correct for a well-formed literal. But for untrusted
input in production code, parameterized queries remain the right tool;
string escaping is best for one-off queries and generated fixtures.

</details>
