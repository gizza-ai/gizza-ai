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
