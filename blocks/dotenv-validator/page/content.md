## About the dotenv validator

This tool **lints a `.env` (dotenv) file** and reports problems without ever
rewriting it. Paste your file into the **.env file contents** box and it parses
each `KEY=VALUE` line — understanding `#` comments, blank lines, single/double
quotes, inline comments and an `export ` prefix — then flags the issues below,
each with a **line number**, a **severity** (error or warning) and a **rule
name**. Everything runs locally in your browser; nothing is uploaded.

### What it checks

- **Duplicate keys** — the same key assigned twice. Most loaders keep only the
  last value, so the earlier one is silently dead.
- **Unquoted values with spaces** — `GREETING=hello world` keeps only `hello`
  in many loaders; wrap the value in quotes. Toggle this off with **Flag
  unquoted values that contain a space**.
- **Bad `${VAR}` interpolation** — an unclosed `${` (missing `}`) or an empty
  `${}`.
- **Undefined references** — `${VAR}`/`$VAR` pointing at a variable never
  defined in the file. Values that come from the shell or CI (like `HOME` or
  `CI`) can be whitelisted in **Allow undefined** so they are not flagged.
- **Syntax nits** — invalid key names (must match `[A-Za-z_][A-Za-z0-9_]*`),
  lowercase keys (the convention is `UPPER_SNAKE_CASE`), whitespace around `=`,
  empty values, unterminated quotes, and CRLF (`\r\n`) line endings.

Interpolation checks (unclosed/empty/undefined) can be turned off with **Check
${VAR} interpolation**. Choose **Report** for a readable list or **JSON** for a
structured `{ok, keys, error_count, warning_count, issues[]}` object you can pipe
into CI.

### Worked example

Given this `.env`:

```
# database
export DB_HOST=localhost
DB_PORT=5432
GREETING=hello world
URL=http://${DB_HOST}:${DB_PORT}/${MISSING}
DB_HOST=127.0.0.1
```

the report flags three things: the **unquoted space** in `GREETING` (a warning),
the **undefined reference** `${MISSING}` on the URL line (a warning), and the
**duplicate key** `DB_HOST` re-assigned on the last line (a warning) — while
`${DB_HOST}` and `${DB_PORT}` resolve fine because both are defined in the file.
Selecting **JSON** returns the same findings as `issues[]` with `ok: true`
(there are no hard errors here, only warnings).

### Limits & edge cases

- **Single-quoted values are literal** — `X='${FOO}'` is not interpolated, so
  `${FOO}` there is never reported as a reference.
- **Backslash-escaped `\$`** and a bare `$` (e.g. a `$5` price) are treated as
  literals, not interpolation.
- **References resolve regardless of order** — a `${VAR}` used before its own
  definition line still counts as defined.
- **`allow_undefined` is case-sensitive** and only affects the
  undefined-reference rule.
- Duplicate keys, lowercase keys, spaces around `=`, empty values and CRLF are
  **warnings** (the file still loads); missing `=`, empty keys, invalid key
  names, unterminated quotes and malformed interpolation are **errors**.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>Does this upload or store my .env file?</summary>

No. The linter is compiled to WebAssembly and runs **entirely in your browser** —
your `.env` text never leaves the page and nothing is sent to a server. You can
safely paste real secrets to check the file's structure; the tool only reads it
to report issues and never rewrites or transmits it.

</details>

<details>
<summary>Why is my `${VAR}` reported as undefined?</summary>

A `${VAR}` (or `$VAR`) reference is flagged when **no line in the same file
defines `VAR`**. If the variable is supplied by the shell or your CI system
(such as `HOME`, `PATH` or `CI`), add its name to the **Allow undefined** box as
a comma-separated list so it is treated as defined. Note the match is
case-sensitive, and single-quoted values are literal — `'${VAR}'` is never
interpolated, so it is never checked.

</details>

<details>
<summary>What's the difference between an error and a warning?</summary>

**Errors** are things most loaders can't parse correctly: a line with no `=`, an
empty key, an invalid key name, an unterminated quote, or malformed `${...}`
interpolation. **Warnings** are things that load but are probably not what you
meant: duplicate keys (last value wins), unquoted values with spaces, lowercase
key names, whitespace around `=`, empty values, and CRLF line endings. In
**JSON** output, `ok` is `true` only when there are zero errors.

</details>

<details>
<summary>Which dotenv syntax does it understand?</summary>

Standard `.env` conventions: `KEY=VALUE` lines, `#` comments (whole-line and
inline after a space), blank lines, an optional `export ` prefix, and single- or
double-quoted values (single quotes are literal, double quotes interpolate).
Keys are expected to be `UPPER_SNAKE_CASE` matching
`[A-Za-z_][A-Za-z0-9_]*`. It does **not** execute or resolve values — it only
diagnoses them.

</details>
