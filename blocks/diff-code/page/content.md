## About this tool

Paste an old and a new version of a function, a config file, a query, or any
other block of text and get the difference back in the shape you need: two
aligned columns, a clean unified patch, one merged word-diff stream, a count
summary, or a structured JSON report.

Two things make the result readable on real code. First, the line alignment is a
**patience diff** — lines that appear exactly once on both sides act as anchors,
so a duplicated `}` or a moved blank line doesn't drag the rest of the comparison
out of step. Second, a changed line pair is refined **inside the line**: only the
tokens that actually changed are marked, using the familiar
`[-removed-]` / `{+added+}` convention.

Everything runs locally in WebAssembly — the snippets never leave the browser.

### Worked example

Comparing

```
fn main() {
    println!("hi");
}
```

with

```
fn main() {
    println!("hello");
    println!("world");
}
```

produces, in the default side-by-side view:

```
 1   fn main() {                     |  1   fn main() {
 2 ~     println!("[-hi-]");         |  2 ~     println!("{+hello+}");
                                     |  3 +     println!("world");
 3   }                               |  4   }

1 line added, 0 lines removed, 1 line changed, 2 lines unchanged
```

The `~` marks a changed pair, `+` a pure insertion, `-` a pure deletion, and the
unchanged prefix `println!("` is left alone because only the string literal moved.
Switch **View** to `stats` on the same input and you get:

```
Lines:      3 left / 4 right
Added:      1
Removed:    0
Changed:    1
Unchanged:  2
Similarity: 50.0%
Word-level:  1 removed / 1 added inside changed lines
```

### Inputs

- **Original code** / **Updated code** — the two sides. Up to 1 MB each.
- **View** — `side-by-side` (default), `unified`, `word-diff`, `stats`, or `json`.
- **Highlight inside changed lines** — word level (default), character level, or off.
- **Ignore case** / **Ignore whitespace** — matching-only options; the output
  always echoes the original text.
- **Context lines** — unchanged lines kept around each change; the rest collapse
  into a `… N unchanged lines …` marker. Default 3.
- **Show line numbers** — per-side numbering in the text views. Default on.
- **Column width** — per-column content width for the side-by-side layout.
  Default 60 characters.

### Limits and edge cases

- **1 MB per side**, enforced as a hard cap; going over returns an error naming
  the limit and the size you sent.
- A changed line pair where either side tokenises past **400 tokens** skips
  intra-line refinement and is marked whole-line instead. This keeps a minified
  bundle or a one-line JSON document from stalling the page.
- **CRLF and LF never differ.** Line endings are normalised before comparison, so
  a file that only changed line endings reports no differences.
- **Tabs expand to 4 spaces** in the side-by-side view only, so the two columns
  stay aligned. Every other view echoes the original characters.
- The `unified` view is deliberately clean — no intra-line markers, no summary
  line — so it stays a valid patch you can pipe into `git apply`.
- Similarity is unchanged lines as a share of the longer side, reported to one
  decimal place.

## FAQ

<details>
<summary>What do the <code>[-…-]</code> and <code>{+…+}</code> markers mean?</summary>

They are the `git diff --word-diff` convention. `[-hi-]` is text present only on
the left (removed), `{+hello+}` is text present only on the right (added).
Everything outside the markers is identical on both sides. In the side-by-side
view each column shows only its own marker type; in the `word-diff` view both
appear inline in one merged stream. Set **Highlight inside changed lines** to
*Off* if you'd rather see whole lines marked without any inline markers.

</details>

<details>
<summary>When should I use character level instead of word level?</summary>

Word level splits a line into identifiers, punctuation, and whitespace runs, so
`color` → `colour` is reported as one whole identifier swapped. Character level
narrows that to the single inserted `u` (`colo{+u+}r`). Character level is the
better choice for typo hunting, renamed symbols that share a stem, and numeric
literals; word level reads better on ordinary code edits, where a per-character
diff of a rewritten expression turns into confetti.

</details>

<details>
<summary>If I turn on ignore case or ignore whitespace, does my code get rewritten?</summary>

No. Both options affect *matching* only. `const B = 2;` and `const b = 2;`
compare equal with ignore-case on, so the line is no longer reported as changed —
but any line that is shown keeps its exact original casing, spacing, and
indentation. Ignore-whitespace collapses runs of spaces and tabs and trims line
ends for the comparison, which is the quick way to tell a reformat apart from a
real edit.

</details>

<details>
<summary>Why did some unchanged lines disappear from the output?</summary>

That's the **Context lines** setting. Only that many unchanged lines are kept
around each change; longer unchanged runs collapse into a `… N unchanged lines …`
marker so a small edit in a big file doesn't bury you. Raise it to see more
surrounding code, or set it to 0 to see only the changes themselves. It applies
to the side-by-side, unified, and word-diff views; `stats` and `json` always
cover the whole input.

</details>

<details>
<summary>Can I apply the result as a patch, or feed it to another tool?</summary>

Yes. The `unified` view emits a standard `--- / +++ / @@` patch with no inline
markers and no trailing summary, so it stays valid input for `git apply` or any
patch-applying tool. For programmatic use pick `json`, which returns the counts,
the similarity, and every row with its line numbers plus the equal/delete/insert
spans inside changed lines.

</details>

<details>
<summary>How large an input can it handle?</summary>

Up to 1 MB per side. That is roughly 25,000 lines of ordinary source, and the
error message names both the limit and the size you sent if you go over. Very
long individual lines are handled by a separate guard: past 400 tokens on either
side of a changed pair, intra-line refinement is skipped and the line is marked
whole. Splitting a minified file onto multiple lines first gives a far more
useful comparison.

</details>
