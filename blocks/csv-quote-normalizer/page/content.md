## About this tool

A CSV is only half a format. The delimiter is obvious; the *quoting* is not, and every exporter picks its own convention. MySQL writes `\"` inside a quoted field. Postgres `COPY` can be told to do either. Word and Google Docs turn `"` into `“` and `”` the moment someone opens the file to fix a typo. A hand-edited row picks up `'single quotes'`, or a stray `"` in the middle of a comment, or a space before the opening quote. Every one of those files looks fine in a text editor and then blows up in pandas, `csv.reader`, Excel's import wizard, or a `COPY … FROM` — usually not with an error, but with a row silently split in the wrong place.

This tool reads the file with a deliberately **tolerant** parser and writes it back out **strict**. Nothing about the input has to be well-formed; everything about the output is.

### What it will read

| In the input | How it is read |
| --- | --- |
| `"x \"y\""` | backslash-escaped quote → a literal `"` in the value |
| `'Boston, MA'` | single-quoted field (when `'` actually opens a field) |
| `“smart quoted”` | curly quotes as field quotes |
| `1, "padded"` | padding before or after a quote, dropped |
| `"He said "hi" to me"` | stray inner quote kept as text |
| `"abc"def` | text after a closing quote merged back into the value |
| `"oops,2` *(EOF)* | unclosed field, closed at end of input |
| `﻿` at the start, blank lines | dropped |

### What it will write

One dialect, chosen by you: which fields are quoted, which character quotes them, how a quote inside them is escaped, which delimiter separates them, and which line ending terminates the row — embedded newlines included, so the whole file is consistent.

### Worked example

Input (the default example on this page):

```text
id,name,note
1,"Ada \"Countess\" Lovelace",fine
2,“Grace Hopper”, "padded, quoted"
3,"He said "hi" to me",ok
```

Output:

```text
id,name,note
1,"Ada ""Countess"" Lovelace",fine
2,Grace Hopper,"padded, quoted"
3,"He said ""hi"" to me",ok
```

Four different quoting conventions went in; one came out. `Grace Hopper` lost its quotes because minimal quoting does not need them, and `padded, quoted` kept its because it contains the delimiter.

### Controls

- **Input delimiter** — `auto` (default) sniffs the most frequent of `,` `;` tab `|` outside quotes on the first logical line, or name one (`comma`, `tab`, `semicolon`, `pipe`, `space`) or give a single character.
- **Output delimiter** — `same` (default), or any delimiter spec to change dialect on the way out.
- **Quote character to read** — `auto` (default) picks double if any straight or curly double quote appears, and single only when a `'` genuinely opens a field, so `it's` in a comment is never mistaken for a quote. `none` treats every quote as literal content.
- **Which output fields to quote** — `minimal` (default) quotes only what must be quoted; `always` quotes everything, which keeps a diff stable and stops a spreadsheet retyping `007` as `7`; `non_numeric` quotes everything that is not a plain decimal number, matching Python's `csv.QUOTE_NONNUMERIC`; `never` quotes nothing, and then needs backslash escaping to stay readable.
- **Output quote character** — `"` (default) or `'`.
- **How to escape a quote inside a field** — `doubled` (`""`, RFC 4180) or `backslash` (`\"`).
- **Read `\"` in the input as an escaped quote** — on by default. Turn it off when backslash is ordinary content.
- **Read curly quotes as field quotes** — on by default. Turn it off to keep `“` and `”` as characters in the value.
- **Line ending** — `LF` (default) or `CRLF`.
- **Result** — the rewritten CSV, or a report of what was repaired.

### The report

Switch **Result** to *A report of what was repaired* and you get an audit instead of a file: the detected input delimiter and quote character, the output dialect, the row and field counts, a warning if the rows are ragged, how many fields were quoted before and after, and every repair grouped by kind with the line numbers it happened on. It is the fastest way to find out *why* a file was breaking your loader.

### Limits and edge cases

The input is capped at 5,000,000 bytes. This tool re-quotes and nothing else: cell values are never trimmed, retyped, rounded or re-cased, rows are never added, dropped or reordered, and a ragged row keeps its length — it is reported, not padded. `quote_style = never` with `escape = doubled` is refused with the offending row and field named, because there would be no way to represent a value containing the delimiter; the fix is backslash escaping or a different policy. The output always ends with one line terminator. Everything runs locally in your browser — the file is never uploaded.

## FAQ

<details>
<summary>My CSV loads fine in a text editor but pandas splits a row in the wrong place. Why?</summary>

Almost always backslash-escaped quotes. An exporter wrote `"x \"y\""`; a strict RFC 4180 reader does not know `\` is an escape, so it sees the `"` after the backslash as the **closing** quote and everything after it as a new field. Paste the file here with default settings: the tolerant parser reads the backslash convention, and the output uses `""` instead, which every strict reader understands.

</details>

<details>
<summary>What is the difference between doubled and backslash escaping?</summary>

They are two ways to put a quote character inside a quoted field. RFC 4180 says double it: `"she said ""hi"""`. MySQL's `LOAD DATA`, older Postgres `COPY` variants and most JavaScript-flavoured writers use a backslash: `"she said \"hi\""`. Neither is wrong, but a reader that expects one and gets the other mis-parses the row. Pick the one your destination expects under **How to escape a quote inside a field** — and leave **Read `\"` in the input as an escaped quote** on so the source convention is understood either way.

</details>

<details>
<summary>Where do the curly quotes come from, and should I keep them?</summary>

From autocorrect. Word, Google Docs, macOS text substitution and most CMS editors rewrite `"` as `“` … `”` as you type, so any CSV that a human opened and edited can come back with typographic quotes wrapping its fields. With **Read curly quotes as field quotes** on (the default) they are treated as quoting and the output gets straight quotes. Turn it off when the curly quotes are real content — a quotation inside a text column, for example — and they will be preserved as characters.

</details>

<details>
<summary>Can I change the delimiter at the same time?</summary>

Yes. Set **Output delimiter** to `tab`, `semicolon`, `pipe`, `space` or any single character. That is safer than a plain find-and-replace, because the switch happens on the parsed fields: a comma *inside* a value stays part of the value, and quoting is recalculated for the new delimiter — so a field that only needed quotes because of a comma often comes out unquoted in a TSV.

</details>

<details>
<summary>What happens to a row with an unclosed quote?</summary>

It is closed at the end of the input rather than swallowing the rest of the file, and the report tells you which line opened it. That is a repair, not a guess about your intent, so check the result: if a quote was opened by accident, everything after it on that line has been folded into one field. The report's line number points straight at the row to fix upstream.

</details>

<details>
<summary>Does it trim spaces, fix headers or pad short rows?</summary>

No — deliberately. This tool changes quoting, escaping, the delimiter and line endings, and nothing else. Padding inside cells is `csv-whitespace-normalizer`; reporting structural problems without rewriting is `csv-structure-validator`. Keeping the jobs separate means you can always tell what a step changed.

</details>

<details>
<summary>Does my file get uploaded?</summary>

No. The parser and writer are compiled to WebAssembly and run inside the page, so the CSV never leaves your browser. For a file on disk you can run the same code offline from the terminal: `gizza tool csv-quote-normalizer input="$(cat data.csv)"`.

</details>
