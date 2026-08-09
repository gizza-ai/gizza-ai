## About this tool

Regular expressions are great at finding things in messy text; spreadsheets are great at
everything after that. This tool joins the two: it runs your pattern over the whole input and
writes **one CSV row per match**, using the pattern's capture groups as columns.

Name your groups — `(?<name>…)` or `(?P<name>…)` — and those names become the header row. A
pattern with plain unnamed groups gets `column1`, `column2`, … instead, and a pattern with no
groups at all produces a single `match` column holding each whole match, so any regex you already
have works without editing.

Everything runs locally in your browser: the text you paste never leaves the page.

### Worked example

Input text:

```
2026-07-20 14:03:11 ERROR auth Failed login for alice
2026-07-20 14:03:15 INFO http GET /health 200
```

Pattern:

```
(?<date>\S+) (?<time>\S+) (?<level>[A-Z]+) (?<module>\S+) (?<message>.*)
```

Output CSV (default settings — comma delimiter, header on, minimal quoting):

```
date,time,level,module,message
2026-07-20,14:03:11,ERROR,auth,Failed login for alice
2026-07-20,14:03:15,INFO,http,GET /health 200
```

Set **Columns** to `level, message` to emit just those two, in that order. Switch the delimiter to
`tab` for TSV, or line endings to CRLF when the file is headed for Excel on Windows.

### What you can control

- **Columns** — pick a subset and reorder it; blank means every group in pattern order.
- **Delimiter** — a single character, `\t`, or the keywords `comma`, `semicolon`, `tab`, `pipe`,
  `colon`, `space`.
- **Header row** — turn it off to append rows to an existing file.
- **Quoting** — *minimal* quotes only fields containing the delimiter, a quote, or a line break;
  *all* quotes every field. Embedded quotes are always doubled (`"` → `""`), per RFC 4180.
- **Line endings** — LF or CRLF.
- **Regex flags** — ignore case (`i`), multiline `^`/`$` (`m`), and dot-matches-newline (`s`).
- **Dedupe and sort** — drop repeated rows, then sort them lexicographically.

### Limits and edge cases

- Input is capped at **1 MB** and **100,000 rows**; larger inputs return an explanatory error
  instead of hanging the tab.
- A capture group that did not participate in a match (an optional group) becomes an **empty
  field**, so every row has the same number of columns.
- If the pattern has named groups, unnamed groups in the same pattern are ignored.
- If nothing matches, you get an error rather than an empty file — that is almost always a pattern
  bug worth seeing.
- Syntax is the Rust `regex` crate's: no backreferences and no lookaround. Character classes,
  non-greedy quantifiers, alternation, anchors, and Unicode classes all work.

## FAQ

<details>
<summary>How do I name the columns in my CSV?</summary>

Name the capture groups. `(?<ip>\S+) (?<status>\d{3})` produces a CSV with the header `ip,status`.
Both syntaxes are accepted — `(?<name>…)` and `(?P<name>…)`. If you would rather keep the pattern
untouched, leave the groups unnamed and the columns come out as `column1`, `column2`, … which you
can rename in your spreadsheet.

</details>

<details>
<summary>What happens to commas, quotes, and newlines inside a captured value?</summary>

They are escaped properly. A field containing the delimiter, a double quote, or a line break is
wrapped in double quotes, and any quote inside it is doubled — so `say "hi", now` becomes
`"say ""hi"", now"`. That is the RFC 4180 convention every spreadsheet understands. Choose
*All fields quoted* if your downstream tool prefers a uniformly quoted file.

</details>

<details>
<summary>Can one row come from text that spans several lines?</summary>

Yes. The pattern is applied to the whole input, not line by line. Turn on **Dot matches newlines**
so `.` crosses line boundaries, and a pattern like `<td>(?<cell>.+?)</td>` will capture a cell whose
content is wrapped over two lines. The captured newline is preserved inside a quoted CSV field.

</details>

<details>
<summary>Why do I get "no matches" when the pattern looks right?</summary>

The usual causes are case (turn on **Ignore case**), anchors (`^` and `$` only match the very start
and end of the text unless **Multiline** is on), and `.` not crossing line breaks (turn on **Dot
matches newlines**). Also remember that backreferences and lookaround are not supported by this
regex engine — a pattern copied from a PCRE tester that uses `(?=…)` will fail to compile and you
will get an "invalid regular expression" error instead.

</details>

<details>
<summary>How do I get a TSV, or a file Excel opens cleanly?</summary>

Set the delimiter to `tab` for TSV. For Excel on Windows, set line endings to CRLF; keep the header
row on so the columns are labelled. If your data contains semicolons and your locale's Excel splits
on semicolons, pick a different delimiter — any single character is allowed.

</details>

<details>
<summary>Is the same thing available on the command line?</summary>

Yes — every tool here is also a CLI command with the same parameters, so a pattern you tuned on
this page can be dropped straight into a script or piped into another tool. The page shows the
exact command for the values you have entered.

</details>
