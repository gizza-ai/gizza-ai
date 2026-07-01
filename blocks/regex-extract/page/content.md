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

## FAQ

<details>
<summary>Why don't lookaheads or backreferences work?</summary>

The engine is the Rust `regex` flavour, which guarantees linear-time matching —
that's why a hostile pattern can never hang the page. The trade-off is that
look-around (`(?=…)`, `(?<=…)`) and backreferences (`\1`) aren't supported;
patterns using them are rejected with a syntax error. Usually a capture group
plus the **Capture group** option achieves the same extraction.

</details>

<details>
<summary>How do I extract just part of each match?</summary>

Wrap the part in parentheses and set **Capture group** to its number — `0` is the
whole match, `1` the first group, and so on. With `(\w+)=(\w+)` and group `2` you
get only the values. A group number larger than the pattern actually has produces
an error telling you how many groups exist; matches where an optional group
didn't participate are skipped.

</details>

<details>
<summary>Why does ^ only match at the very start of my text?</summary>

By default `^` and `$` anchor the whole input. Tick **Multiline** to anchor at
the start and end of each line instead, and **Dot matches newline** if you want
`.` to span line breaks — the two flags are independent.

</details>

<details>
<summary>Does "Unique matches only" change the order of results?</summary>

No — deduplication keeps first-seen order, so the list still reflects where each
distinct value first appeared in the text. Everything runs locally in your
browser; neither the text nor the pattern is uploaded.

</details>
