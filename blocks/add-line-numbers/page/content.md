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
