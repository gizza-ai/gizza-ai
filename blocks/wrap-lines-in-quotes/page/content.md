## About this tool

**Wrap Lines in Quotes** puts your chosen quotes or brackets around every line
of text and, optionally, adds a trailing separator — so a pasted column of
values becomes a ready-to-use SQL `IN (…)` list, JSON array, or CSV row. It all
runs **locally in your browser** via WebAssembly; nothing is uploaded.

- **Wrap style** — pick double `"…"`, single `'…'`, backticks `` `…` ``,
  parentheses `(…)`, square `[…]`, curly `{…}`, angle `<…>`, or guillemets
  `«…»`. Choose **Custom** to type your own opening/closing delimiters (leave
  the closing one empty to mirror the opening one).
- **Trailing separator** — appended after each wrapped line. Type `,` to build a
  comma-separated list. By default the **last** line omits it, so the result is a
  valid `IN (…)` / array body; tick *Separator after the last line too* to keep it.
- **Skip blank lines** — on by default: blank / whitespace-only lines pass
  through unchanged (not wrapped, no separator).
- **Trim whitespace** — strips leading/trailing spaces on each line before wrapping.
- **Escape the delimiter inside lines** — backslash-escapes the quote (and any
  `\`) that already appears in a line, so a value like `5" pipe` becomes a valid
  `"5\" pipe"` instead of breaking your string.

### Worked example

Paste three values, choose **single quotes**, and set the separator to `,`:

```
apple
banana
cherry
```

You get a drop-in SQL `IN` list body:

```
'apple',
'banana',
'cherry'
```

Wrap the same lines in **square brackets** with no separator and each becomes
`[apple]`, `[banana]`, `[cherry]`.

### Handy for

- Building `WHERE col IN ('a','b','c')` clauses from a spreadsheet column.
- Turning a list into a JSON/Python array of strings.
- Quoting shell arguments or config values line by line.

## FAQ

<details>
<summary>How do I make a SQL IN list or a JSON array?</summary>

Paste one value per line, pick your quote style (single quotes for SQL, double
for JSON), and set the **Trailing separator** to `,`. Every line is quoted and
comma-separated, and the **last** line drops the trailing comma by default, so
you can paste the result straight inside `IN ( … )` or `[ … ]` without a syntax
error. Wrap the whole block in parentheses or brackets yourself.

</details>

<details>
<summary>What happens to blank lines?</summary>

With **Skip blank lines** on (the default), any empty or whitespace-only line is
left exactly as it was — it isn't quoted and doesn't receive a separator, so the
surrounding lines still form a clean list. Turn the option off to wrap every
line, including blank ones (a blank line then becomes an empty pair like `""` or
`[]`). The result also reports how many of the total lines were wrapped.

</details>

<details>
<summary>My values contain quotes — won't that break the output?</summary>

Enable **Escape the delimiter inside lines**. It backslash-escapes any
occurrence of the delimiter — and any literal backslash — inside each line
before wrapping, so `5" pipe` becomes `"5\" pipe"` and `a\b` becomes `"a\\b"`.
That keeps double-quoted JSON/SQL string literals valid. Without it, inner
quotes are left as-is (useful when you know they're already safe).

</details>

<details>
<summary>Can I use my own brackets or a multi-character wrapper?</summary>

Yes. Set **Wrap style** to **Custom** and type the opening delimiter (e.g.
`<!--`) and closing delimiter (e.g. `-->`). If you leave the closing field
empty, it mirrors the opening one, so typing `|` wraps each line as `|value|`.
Multi-character delimiters like `<<` / `>>` work too.

</details>

## Limits & notes

- Lines are split on newlines only; the tool never re-wraps or reflows long
  lines. A trailing newline in the input does not create an extra empty line.
- **Escape** covers only the delimiter characters and backslashes — it is not a
  full JSON/CSV encoder (it won't escape tabs or control characters).
- Everything is processed in-browser; large pastes are limited only by your
  device's memory.
