## About this tool

**Head** keeps just the first lines of whatever text you paste — the same thing
the Unix `head -n` command does, but in your browser with no install. Paste a
log, a CSV, a giant export, or any block of text and instantly get back only the
top rows.

### What you can do

- **Keep the first N lines.** Set *Number of lines to keep* (default 10) to take
  exactly that many leading lines.
- **Skip a header row.** Use *Skip leading lines* to drop lines from the very
  start before counting — for example skip 1 to ignore a CSV header.
- **Number each line.** Turn on *Number each line* to prefix every kept line with
  its original 1-based line number and a tab, like `cat -n` or `nl`.

Windows line endings (CRLF) and a trailing newline are preserved, so the head of
a file keeps its original structure.

### Private by design

Everything runs locally in your browser via WebAssembly. Your text is never
uploaded to a server — there's nothing to sign up for and nothing leaves your
machine.

## FAQ

<details>
<summary>What if I ask for more lines than the text has?</summary>

You simply get the whole text back — no error and no padding. Asking for the
first 100 lines of a 20-line paste returns all 20 lines, same as `head -n 100`
on a short file.

</details>

<details>
<summary>Is there a maximum line count?</summary>

Yes: the count is clamped to the range 1 – 1,000,000. Leaving it empty uses the
classic `head` default of 10 lines. Values outside the range are pulled back to
the nearest bound rather than rejected.

</details>

<details>
<summary>How does skipping lines interact with line numbering?</summary>

Numbering always reflects the **original** position in your text. With *Skip
leading lines* = 1 and *Number of lines* = 2, you get lines 2 and 3 prefixed
`2⇥` and `3⇥` (a tab after the number, like `cat -n`) — not renumbered from 1.

</details>

<details>
<summary>Does it mangle Windows line endings or the final newline?</summary>

No. Lines are split on `\n` and any `\r` from CRLF endings is carried through
untouched, and a trailing newline is kept only if your input ended with one —
so the head of a file preserves the file's original structure byte-for-byte.

</details>
