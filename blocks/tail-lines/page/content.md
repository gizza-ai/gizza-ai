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
