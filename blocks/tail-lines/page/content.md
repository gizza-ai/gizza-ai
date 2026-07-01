## About this tool

**Tail** keeps just the last lines of whatever text you paste — the same thing
the Unix `tail -n` command does, but in your browser with no install. Paste a
log, a CSV, a giant export, or any block of text and instantly get back only the
bottom rows.

### What you can do

- **Keep the last N lines.** Set *Number of lines to keep* (default 10) to take
  exactly that many trailing lines.
- **Skip a footer row.** Use *Skip trailing lines* to drop lines from the very
  end before counting — for example skip 1 to ignore a totals or summary line.
- **Number each line.** Turn on *Number each line* to prefix every kept line with
  its original 1-based line number and a tab, like `cat -n` or `nl`, so the
  numbers reflect each line's real position near the end of the input.

Windows line endings (CRLF) and a trailing newline are preserved, so the tail of
a file keeps its original structure.

### Private by design

Everything runs locally in your browser via WebAssembly. Your text is never
uploaded to a server — there's nothing to sign up for and nothing leaves your
machine.

### FAQ

<details>
<summary>How do "skip" and "count" interact?</summary>

Skip is applied first: the last *skip* lines are dropped from the end, then the last *count* lines of what remains are kept. So `count=10, skip=1` gives you rows 11-through-2 from the bottom — perfect for ignoring a totals/footer row.

</details>

<details>
<summary>What do the line numbers refer to when numbering is on?</summary>

Each kept line is prefixed with its 1-based position **in the original text** (plus a tab), not a fresh 1..N sequence. If your input had 500 lines and you keep the last 10, they're numbered 491-500, so you can locate them in the source file.

</details>

<details>
<summary>Is there a maximum line count?</summary>

Count is clamped to the range 1 to 1,000,000. Asking for more lines than the text contains simply returns the whole text — no error, same as `tail -n` on a short file.

</details>

<details>
<summary>Does it mangle Windows (CRLF) files or the trailing newline?</summary>

No — CRLF endings are preserved on each line, and a final newline is kept only if the original input ended with one, so the tail is byte-faithful to the source.

</details>
