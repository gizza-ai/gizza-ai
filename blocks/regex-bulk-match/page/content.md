## About this tool

A regular expression tester is useful for one sample. Real cleanup and validation jobs usually
start with a list: hundreds of IDs, email addresses, log lines, filenames, or pasted rows from a
spreadsheet. This tool applies **one regex to every line** and reports the verdict for each line,
so you can see both the matches and the failures without losing line numbers.

The report includes totals, the matched substring, capture groups, and optional byte offsets. Named
capture groups such as `(?<year>\d{4})` keep their names in text, JSON, and CSV output. Everything
runs locally in your browser; the pasted data is not uploaded.

### Worked example

Lines:

```
ada@example.com
not-an-email
bo@test.org
```

Pattern:

```
^[\w.+-]+@([\w-]+\.[\w.]+)$
```

With **require whole-line match** and **capture groups** on, the text report shows two matches and
one failure:

```
Lines tested: 3
Matched: 2
Not matched: 1

line 1: MATCH    "ada@example.com" | 1=example.com
line 2: NO MATCH "not-an-email"
line 3: MATCH    "bo@test.org" | 1=test.org
```

Switch to **CSV** when you want one row per input line and one column per capture group, or **JSON**
when another script will consume the result.

### What you can control

- **Pattern** — Rust regex syntax. Use anchors (`^...$`) or turn on whole-line match for validation.
- **Rows to show** — all rows, only matching rows, or only non-matching rows. Totals always include
  every tested row.
- **Output format** — readable text, structured JSON, or CSV.
- **Capture groups** — include unnamed groups as `1`, `2`, and named groups by their names.
- **Show match offsets** — add start/end byte offsets to text and CSV. JSON always carries offsets.
- **Trim each line** and **skip blank lines** — enabled by default for pasted lists.
- **Ignore case** and **dot matches newlines** — common regex flags exposed as checkboxes.
- **Max lines** — default 1,000; maximum 20,000. Extra lines are skipped and the report says it was
  truncated.

### Limits and edge cases

- This uses Rust's `regex` engine: no backreferences or look-around, but matching is linear-time and
  avoids catastrophic backtracking.
- Offsets are byte offsets in the tested line. For plain ASCII they match character positions; for
  emoji or other multi-byte characters they can differ.
- `.` does not match line breaks unless **dot matches newlines** is enabled. Because input is tested
  line by line, that flag is mostly useful for pasted records that contain embedded newlines after
  another tool has escaped or joined them.
- Blank lines are skipped by default. Turn off **skip blank lines** when blank rows are meaningful.
- The **whole-line match** option wraps your pattern as `^(?:pattern)$`, so alternations such as
  `yes|no` are anchored as a group.

## FAQ

<details>
<summary>How is this different from a normal regex tester?</summary>

A normal tester highlights matches in one text blob. This tool treats each line as a separate case
and gives every line a pass/fail verdict. That makes it better for validating pasted lists: you can
filter to only failures, keep original line numbers, and export the result as CSV or JSON.

</details>

<details>
<summary>Can I use named capture groups?</summary>

Yes. Rust regex named groups use the `(?<name>...)` syntax. Named groups appear by name in the text
report and as named columns in CSV output; unnamed groups use `group_1`, `group_2`, and so on.

</details>

<details>
<summary>Why does my PCRE pattern fail?</summary>

Rust regex deliberately omits backreferences and look-around so matching stays predictable and
linear-time. Patterns with `(?=...)`, `(?<=...)`, or `\1` return an invalid-pattern error. Rewrite
the pattern without those features, or use a PCRE-specific tester when that syntax is required.

</details>

<details>
<summary>What does whole-line match do?</summary>

It makes the entire tested line satisfy the pattern. Without it, `\d+` matches `abc123` because the
digits occur somewhere inside the line. With whole-line match on, `abc123` fails unless your pattern
accounts for the letters too.

</details>

<details>
<summary>Why are positions called byte offsets?</summary>

The Rust regex engine reports byte offsets. For English letters and digits, byte offsets are the same
as character positions. For multi-byte Unicode characters, a visual character may occupy more than
one byte, so offsets can look larger than the number of visible characters before the match.

</details>
