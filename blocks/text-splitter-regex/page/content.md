## About this tool

Most splitters ask for a single character. Real pasted data rarely cooperates: columns are
separated by *runs* of spaces and tabs, a list mixes commas and semicolons, records are divided by
blank lines, or the delimiter is a word rather than a symbol. This tool takes a **regular
expression as the separator** — everything between matches of your pattern becomes a part.

Note the direction: the pattern describes the **delimiter**, not the data. `\s+` splits on
whitespace; it does not extract whitespace. If you want the matches themselves, that is a
different job (matching, not splitting).

Add a **field pattern** and each row is split again, so delimited text becomes a real table you
can render as CSV, TSV or nested JSON. Everything runs locally in your browser — the text you
paste never leaves the page.

### Worked example

Input text:

```
host:   web1
region: eu-west
role:   api
```

Split pattern `\n`, field pattern `\s*:\s*`, output **CSV**:

```
host,web1
region,eu-west
role,api
```

Leave the field pattern blank and the same input with pattern `\n` gives one line per row instead.
Switch the output to **JSON array** and you get `[["host", "web1"], …]` — nested arrays once a
field pattern is set, a flat array of strings without one.

### What you can control

- **Split pattern** — the separator regex (Rust regex syntax). `\s+` for runs of whitespace,
  `[,;|]` to accept several delimiters at once, `\n{2,}` for blank-line-separated paragraphs,
  `\s*,\s*` for commas with sloppy spacing, ` -- ` for a multi-character separator.
- **Field pattern** — an optional second regex applied to every row, producing columns.
- **Output format** — one part per line (default), JSON array, CSV, TSV, a numbered list, or all
  parts joined by a separator of your choice.
- **Join separator** — used only by the *joined* output format; the escapes `\n`, `\t`, `\r` and
  `\\` are recognised, so `\n\n` puts a blank line between parts.
- **Max splits** — stop after N splits and keep the remainder intact as the final row. `1` on
  `ERROR: disk full: /dev/sda1` with pattern `:\s*` yields `ERROR` and `disk full: /dev/sda1`.
- **Regex flags** — ignore case (`i`), multiline `^`/`$` (`m`), and dot-matches-newline (`s`).
  They apply to both patterns.
- **Trim each part** and **remove empty parts** — clean up leading, trailing and repeated
  separators.

### Limits and edge cases

- Input is capped at **200,000 characters** and **100,000 parts**; anything larger returns an
  explanatory error instead of hanging the tab.
- A separator at the very start or end of the text produces an **empty part** on that side — that
  is standard split behaviour. Turn on *remove empty parts* to drop them.
- A pattern that can match the empty string (`b*`, `\b`) matches at every position and splits
  between every character; that usually hits the parts limit and is nearly always a pattern bug.
- An empty split pattern is rejected — splitting on nothing is meaningless. To get one character
  per line, use a character-splitting tool instead.
- With *remove empty parts* on, a row whose fields are all empty is dropped entirely; if that
  removes everything, you get an error rather than blank output.
- The regex engine is linear-time and has no backtracking, so catastrophic-backtracking patterns
  cannot freeze the page — but it also does not support backreferences or lookaround.

## FAQ

<details>
<summary>How is this different from splitting on a plain character?</summary>

A plain delimiter has to match exactly. If your data is separated by two spaces in one place and a
tab in another, a literal split leaves empty parts and ragged columns. The pattern `\s+` treats any
run of whitespace as one separator, and `[,;|]` accepts three different delimiters in the same
input. Anything you can describe as a regex works — including multi-character separators like
`, ` or ` -- `, and word separators like `\s+then\s+`.

</details>

<details>
<summary>How do I split into columns as well as rows?</summary>

Set the **split pattern** to whatever divides your records — usually `\n` — and the **field
pattern** to whatever divides the values inside a record, such as `\s*:\s*`, `\t` or `\s{2,}`.
Then pick CSV, TSV or JSON as the output format. Without a field pattern the tool produces a
one-dimensional list of parts.

</details>

<details>
<summary>Why do I get an empty first or last part?</summary>

Because the text begins or ends with the separator. Splitting `,a,b,` on `,` produces four parts:
an empty one, `a`, `b`, and another empty one. That is correct split behaviour, not a bug — turn on
**remove empty parts** to drop them, and **trim each part** if the parts also carry stray spaces.

</details>

<details>
<summary>Which regex syntax is supported?</summary>

Rust's `regex` syntax: character classes, quantifiers, groups, alternation, anchors, Unicode
classes like `\p{L}`, and inline flags. It is a linear-time engine, so **backreferences and
lookaround (`(?=…)`, `(?<=…)`) are not supported** — a pattern using them returns an
"invalid pattern" error naming the problem. The `i`, `m` and `s` flags are available as
checkboxes rather than inline flags, though inline forms like `(?i)` work too.

</details>

<details>
<summary>What does max splits do?</summary>

It caps how many times the input is cut into rows, keeping everything after the last cut as the
final part. This is the classic "split off the first N fields" behaviour: with pattern `:\s*` and
max splits `1`, the line `ERROR: disk full: /dev/sda1` becomes `ERROR` and `disk full: /dev/sda1`
instead of three parts. `0` means unlimited. Field splitting is never capped.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The splitter is compiled to WebAssembly and runs entirely inside your browser tab, so the text
never leaves your machine. The same engine is available offline through the command line, which is
useful for files bigger than the page's limit.

</details>
