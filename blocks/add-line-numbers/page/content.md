## About this tool

**Add Line Numbers** prefixes every line of your text with a sequential
number — the same job as `nl` or `cat -n`, but right in your browser.

- **Start number** — the number given to the first line (default `1`). Use any
  whole number, including `0` or a negative value.
- **Step** — the increment between consecutive numbers (default `1`). Set it to
  `10` for `10, 20, 30…`.
- **Separator** — the text placed between the number and the line content. It
  defaults to a **tab**; type `. `, `: `, ` | `, or anything you like.
- **Number alignment** — *spaces* right-aligns the numbers so they line up
  (` 9`, `10`), *zeros* pads them (`09`, `10`), and *none* leaves them as-is.
- **Skip blank lines** — like `cat -b`, blank (whitespace-only) lines are left
  untouched and don't consume a number.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Numbering a snippet of code or a list for reference in review comments.
- Producing `1.`, `2.`, `3.` style ordered lists from plain lines.
- Turning a plain log or checklist into a numbered one.

## FAQ

<details>
<summary>Can numbering start at 0 or a negative number?</summary>

Yes. **Start number** accepts any whole number, including `0` and negatives, so
you can produce `0, 1, 2…` or `-2, -1, 0…`. Only the **step** is restricted: it
must be at least `1` (a step of `0` or a negative step is rejected with an
error). Alignment still works with negative starts — the minus sign counts
toward the column width.

</details>

<details>
<summary>How do I get 10, 20, 30 style numbering?</summary>

Set **Start number** to `10` and **Step** to `10`. The first line gets `10`,
the second `20`, and so on. Any start/step combination of whole numbers works,
e.g. start `100`, step `5` gives `100, 105, 110…`.

</details>

<details>
<summary>What is the difference between the spaces, zeros, and none alignments?</summary>

They control how numbers are padded to a common column width, which is the
width of the largest number in the output. *Spaces* right-aligns (`&nbsp;9`,
`10`), *zeros* left-pads with zeros (`09`, `10`), and *none* writes each number
as-is (`9`, `10`), so columns may not line up past line 9.

</details>

<details>
<summary>How does "skip blank lines" treat lines that contain only spaces?</summary>

With **Skip blank lines** enabled (the `cat -b` behavior), any line that is
empty *or contains only whitespace* is passed through unchanged — it gets no
number and doesn't consume one, so the next non-blank line continues the
sequence. The result also reports how many of the total lines were numbered.

</details>
