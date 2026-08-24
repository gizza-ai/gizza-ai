## About this tool

A codemod is a mechanical, repeatable edit applied across a whole set of files — renaming a
symbol, reshaping a date format, swapping an import path, redacting a field. Doing it by hand in
a dozen files is slow and easy to get subtly wrong; doing it with a one-line shell pipeline is
fast and gives you no chance to look before it lands.

This tool sits in between. Paste the files you want to change as one bundle, write a single
regular expression and its replacement, and get back a **unified diff** showing exactly which
lines move in which files. Nothing is written anywhere — the diff is a preview you read, copy, or
feed to `git apply`. Everything runs locally in your browser, so the code you paste never leaves
the page.

### How the bundle is split

The paste is cut into files at **marker lines**. By default (`Auto-detect`) any of these work,
and you can mix them in one paste:

| Marker | Example |
| --- | --- |
| `equals` | `=== src/a.js ===` |
| `dashes` | `--- src/a.js ---` |
| `arrow` | `==> src/a.js <==` (what `tail -n +1 src/*.js` prints) |
| `comment` | `# file: src/a.js` or `// file: src/a.js` |

If your bundle uses something else, pick **Custom pattern** and supply a marker regex with one
capture group for the path, such as `^@@@ (\S+)$`. Pick **No markers** to treat the whole paste as
a single file. Any content before the first marker is treated as a file named `input`.

A quick way to build a bundle on the command line:

```
tail -n +1 src/*.js
```

### Worked example

Paste this, with **Find** `\boldName\b` and **Replace with** `newName`:

```
=== src/a.js ===
const oldName = 1;
use(oldName);
=== src/b.js ===
const other = 2;
```

The diff preview comes back as:

```
# 2 replacements in 1 of 2 files
--- a/src/a.js
+++ b/src/a.js
@@ -1,2 +1,2 @@
-const oldName = 1;
-use(oldName);
+const newName = 1;
+use(newName);
```

`src/b.js` has no match, so it does not appear at all — switch on **List untouched files too**
to see it reported as `# unchanged: src/b.js`. Switch **Output** to *Full rewritten files* to get
the whole bundle back with the edit applied, or to *JSON report* for per-file replacement counts.

### Capture groups

The replacement understands `$1`, `$2`, … for numbered groups, `${name}` for named groups
(`(?P<name>…)`), and `$$` for a literal dollar sign. Reformatting American dates to ISO is a
one-liner: find `(\d{2})/(\d{2})/(\d{4})`, replace with `$3-$1-$2`. Braces are worth using
whenever a digit could run into the following text — `${1}0` means "group 1 then a zero", while
`$10` means "group 10".

### Matching options

* **Plain text** — escape the pattern and insert the replacement verbatim, so `.` and `$1` mean
  themselves. Use it when your search string is code, not a pattern.
* **Ignore case (i)**, **^ and $ match every line (m)**, **. matches newlines (s)** — the usual
  regex flags, applied to the whole pattern. You can also write them inline, e.g. `(?i)todo`.
* **Whole word only** — wraps the pattern in word boundaries, so `id` will not match inside `idx`.
* **Replace every match (g)** — on by default. Turn it off to change only the *first* match in
  each file, which is handy for one-per-file headers.

### Limits

* Up to **1,000,000 characters** and **2,000 files** in one paste; larger inputs are rejected with
  an error rather than silently truncated. Split the work into batches.
* Patterns use **Rust regex syntax**: character classes, named groups `(?P<name>…)`, non-greedy
  quantifiers and Unicode classes all work. Backreferences inside the pattern (`\1`) and
  lookaround (`(?=…)`, `(?<=…)`) are **not** supported — that is the deliberate trade that keeps
  matching linear-time, so no pattern can hang the page.
* Line endings are preserved exactly as pasted, including CRLF and a missing final newline (which
  the diff marks with `\ No newline at end of file`).
* The diff is line-based. A change that rewrites an enormous, completely different block falls
  back to one coarse "remove all, add all" hunk instead of a fine-grained alignment.

## FAQ

<details>
<summary>Can I paste the output straight into <code>git apply</code>?</summary>

Yes. The diff uses standard `--- a/path` / `+++ b/path` headers and `@@` hunks, and the leading
`#` summary line is treated as preamble, so `git apply` and `patch -p1` both accept it. The paths
in the diff are exactly the paths from your marker lines, so make sure those are repo-relative
before you apply.

</details>

<details>
<summary>Why did my lookahead or backreference pattern fail?</summary>

The matcher is the Rust `regex` engine, which has no backtracking. That guarantees every pattern
runs in time linear in the input — no pattern can lock up the page — but it also means
lookaround (`(?=…)`, `(?!…)`, `(?<=…)`) and backreferences inside the pattern (`\1`) are not
available. You can usually restructure the pattern with capture groups: instead of
`foo(?=bar)` → `newfoo`, use `foo(bar)` → `newfoo$1`.

</details>

<details>
<summary>How do I replace text that spans several lines?</summary>

Turn on **. matches newlines (s)** so `.` crosses line boundaries, then a pattern like
`<a>.*?</a>` will match across lines. Use the non-greedy `.*?` rather than `.*` unless you really
want the match to run to the last occurrence in the file. **^ and $ match every line (m)** is the
complementary flag: it makes the anchors line-based instead of file-based.

</details>

<details>
<summary>Nothing changed — what went wrong?</summary>

The summary line tells you: `# 0 replacements in 0 of N files`. If N is 1 when you pasted several
files, your markers were not recognised — check the **File markers** setting, or pick
*No markers* deliberately. If N looks right, the pattern itself did not match: try **Ignore
case**, or switch on **Plain text** if your search string contains regex metacharacters such as
`.`, `(`, `[` or `$`.

</details>

<details>
<summary>Is anything uploaded?</summary>

No. The tool is compiled to WebAssembly and runs entirely inside your browser tab. The files you
paste, the pattern and the diff never travel to a server, which makes it safe for private source
code and for logs you are redacting.

</details>

<details>
<summary>Can I run this from the command line?</summary>

Yes — the same block ships in the `gizza` CLI, so you can pipe a bundle through it in a script or
call it from an agent. The generated example above the FAQ is copy-paste runnable, and
`gizza describe regex-codemod` prints every parameter with its default.

</details>
