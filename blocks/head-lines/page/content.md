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
