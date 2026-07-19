## About this tool

Text Pipeline Playground lets you paste text and run it through a small chain of safe transforms: grep lines, reject noise, regex-replace fields, trim, change case, sort, dedupe, take a head/tail sample, split records, and join the result back together. It is inspired by command-line pipelines and interactive text wranglers, but it deliberately does **not** execute Python, shell, JavaScript, or user code.

Write one operation per line in the **Pipeline** box. Operations run from top to bottom, so you can build a repeatable recipe such as:

```text
grep ERROR
replace /^\S+ ERROR /!! /
sort
unique
```

For this input:

```text
2024-01-01 INFO started
2024-01-01 ERROR disk full
2024-01-02 ERROR disk full
2024-01-02 WARN low memory
2024-01-03 ERROR timeout
```

The output is:

```text
!! disk full
!! timeout
```

Supported operations:

- `grep PATTERN` keeps matching lines; `reject PATTERN` drops matching lines.
- `replace /old/new/` runs a regex replacement on each line, including `$1` backreferences. Any delimiter works, for example `replace |/|-|`.
- `prefix TEXT`, `suffix TEXT`, `lower`, `upper`, and `trim` map every line.
- `sort`, `sort -r`, `unique`, `head N`, `tail N`, and `reverse` reshape the line list.
- `split SEP` splits each line into more lines (no separator means whitespace); `join SEP` merges all lines into one line.

Turn on **Regex mode** when `grep`/`reject` should use regular expressions instead of literal substring matching. `replace` always uses regular expressions. **Case-insensitive matching** applies to grep, reject, and replace. **Max output lines** caps runaway output; **On a bad pipeline line** can stop with an error or skip malformed operations.

### Limits

The pipeline is line-oriented and text-only. It does not run arbitrary Python code, call APIs, read files, or preserve binary data. Rust regex syntax is used, so look-around and backreferences in the *pattern* are not supported, but capture groups and `$1` replacements are. Output is capped at the configured line limit after the pipeline finishes.

## FAQ

<details>
<summary>Does this run Python or shell code?</summary>

No. The name comes from Python-style text wrangling workflows, but this tool is a fixed, declarative set of operations implemented in Rust/WebAssembly. It never evaluates arbitrary code, imports packages, opens files, or starts subprocesses. That makes it safe to run entirely in the browser.

</details>

<details>
<summary>When should I turn on Regex mode?</summary>

Use Regex mode when `grep` or `reject` needs anchors, character classes, alternation, or capture-style patterns, such as `^ERROR|WARN` or `\b\d{4}-\d{2}-\d{2}\b`. Leave it off for plain substring searches — literal mode is easier for text containing punctuation like `.` or `?`. The `replace /old/new/` operation always treats `old` as a regex.

</details>

<details>
<summary>How do I remove duplicate lines?</summary>

Add `unique` after any filtering or mapping step. It keeps the first copy of each line and drops later duplicates. If you want duplicates adjacent before deduping, run `sort` first and then `unique`; if original order matters, use `unique` without sorting.

</details>

<details>
<summary>What happens if one pipeline line is wrong?</summary>

With **On a bad pipeline line = Stop**, the tool returns an error that names the failing line and explains what was expected, for example `pipeline line 2: head expected a line count`. With **Skip**, malformed lines are ignored and the remaining valid operations still run. Stop is safer for saved recipes; Skip is useful while experimenting.

</details>

<details>
<summary>Can I process CSV or JSON with this?</summary>

You can do simple line-oriented cleanup such as filtering rows, replacing delimiters, extracting a column with a regex, or turning lines into bullets. For structured CSV/JSON parsing, use a dedicated CSV or JSON tool instead — this playground treats input as plain text and does not understand quoting, nested objects, or schemas.

</details>
