## About this tool

**Regex Extract** runs a regular expression over a block of text and returns
**every match** it finds, in order. It is the fast way to pull structured pieces
— IDs, codes, emails, prices, tags — out of unstructured text without writing a
script.

- **Match flags**: tick **Ignore case** for case-insensitive matching,
  **Multiline** so `^` and `$` anchor at the start and end of each line, and
  **Dot matches newline** so `.` also spans line breaks.
- **Capture groups**: set **Capture group** to a number to return a specific
  parenthesised sub-group instead of the whole match (`0` = the whole match).
  For example, the pattern `(\w+)=(\w+)` with capture group `2` returns just the
  values.
- **Deduplicate**: tick **Unique matches only** to collapse repeats, keeping
  first-seen order.

The syntax is the [Rust `regex`](https://docs.rs/regex/) flavour — a clean,
linear-time engine with no catastrophic backtracking.

Everything runs **locally in your browser** via WebAssembly — your text and your
pattern are never uploaded.

### Handy for

- Pulling every occurrence of a code, ticket ID, or token out of a log or
  document.
- Extracting one field from many lines via a capture group.
- Quickly testing what a regular expression actually matches against real input.
