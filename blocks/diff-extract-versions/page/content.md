## About this tool

Sometimes the patch is all you were sent. A `.patch` attachment on a mailing list, a diff pasted into
a chat thread, a code-review snippet, a fragment in a bug report, a hunk in a CI log — and no copy of
the file it came from. Every other diff tool asks you to supply the original file first and then
applies the patch to it. This one works the other way round: it reads the diff by itself and rebuilds
the two file versions the diff describes, using the context lines the diff already carries.

Paste the output of `git diff`, `git show`, `git format-patch`, `diff -u`, or `svn diff`. Mail
preambles, `index` and file-mode lines, `similarity index` headers and `git format-patch` signature
blocks are ignored, so you can paste a whole patch email without trimming it first.

### Worked example

Input diff:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
     let x = 1;
+    let y = 2;
     println!("{x}");
 }
```

Output with **Both versions, labelled**:

```text
===== BEFORE: src/main.rs =====
fn main() {
    let x = 1;
    println!("{x}");
}
===== AFTER: src/main.rs =====
fn main() {
    let x = 1;
    let y = 2;
    println!("{x}");
}
```

Choose **Original (before) text only** to get just the first block with no banner — copy-paste-ready,
byte for byte, including tabs and CRLF line endings.

### What a diff cannot tell you

A diff records the neighbourhood of each change, not the file. With the usual three lines of context,
everything between two hunks and everything after the last hunk is simply absent from the patch, and
no tool can invent it. Rather than silently joining unrelated regions together, this one accounts for
what is missing:

```text
   | [... 8 lines not in the diff (lines 1-8) ...]
 9 | nine
10 | ten
```

Switch the gap handling to **Omit** when you only want the recoverable fragments spliced together, or
to **Refuse a partial reconstruction** when a half-file result would be worse than an error. If you
control how the diff is produced, `git diff -U100000` embeds the whole file as context and the
reconstruction becomes exact.

## Options and limits

- Diffs up to 1 MB per run.
- **Multi-file patches** work as-is: each file gets its own banner. The file filter picks one path by
  exact path, bare filename, substring, or a `*`/`?` glob such as `src/*.rs`.
- **Created and deleted files** (`/dev/null` on one side) reconstruct as an empty text on that side,
  and the JSON report tags them `added` or `deleted`. Renames are reported as `renamed` with both
  paths.
- **Wrong `@@` counts** do not fail the parse — the hunk body is authoritative, the counts are
  recounted, and the JSON report sets `header_counts_corrected`.
- `\ No newline at end of file` is honoured per side, and CRLF endings survive because everything
  after the leading marker character is copied byte for byte.
- **Binary patches** and **combined merge diffs** (`@@@`, three columns) carry no single before-text
  and are reported as such rather than silently skipped.
- Context format (`diff -c`) and normal `diff` output are not unified diffs and are rejected with a
  message saying which flag to re-run with.
- Everything runs in your browser tab as WebAssembly; the diff you paste is not uploaded anywhere.

## FAQ

<details>
<summary>Do I need the original file?</summary>

No — that is the whole point of this tool. It reconstructs both versions from the patch alone. If you
*do* have the original file and want the patched result, that is applying a patch, and a dedicated
apply-patch tool is the better fit.

</details>

<details>
<summary>Why does my output start with a "lines not in the diff" marker?</summary>

Because the first hunk does not begin at line 1. A diff carries only the context around each change,
so the lines before the first hunk were never in the text you pasted. The marker counts exactly how
many lines are missing and which line numbers they occupied, so you can splice the result back into a
real file with confidence.

</details>

<details>
<summary>Can I get the whole file back instead of fragments?</summary>

Only if the diff carries the whole file. Re-generate it with full context — `git diff -U100000` — and
the reconstruction covers everything up to the last line the diff mentions. Nothing after the final
hunk is knowable from any diff, because a diff never records how long the file is.

</details>

<details>
<summary>What does the JSON output contain?</summary>

One entry per file: both paths, the status (`modified`, `added`, `deleted`, `renamed`, `binary`), the
hunk count, added and removed line counts, a `complete` flag, the exact missing line ranges for each
side, whether the header counts had to be corrected, the trailing-newline state, and the two
reconstructed texts. It is the machine-readable version of the same result.

</details>

<details>
<summary>My diff came from an email and it will not parse.</summary>

Mail clients word-wrap long lines, which destroys the leading `+`/`-`/space marker that every hunk
line must start with. The error message names the exact line so you can unwrap it. Lines that merely
lost a single trailing space (a common transformation for blank context lines) are repaired
automatically, since that fix is unambiguous.

</details>

<details>
<summary>Which diff dialects are supported?</summary>

Unified diff in its common spellings: `git diff` / `git show` (with `diff --git`, `index`, mode,
rename and `new file` headers), `git format-patch` mail patches, POSIX `diff -u` with timestamped
`---`/`+++` headers, and `svn diff` with its `Index:` headers. The `a/` and `b/` prefixes are stripped
for display.

</details>
