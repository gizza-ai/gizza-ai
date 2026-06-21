## About this tool

**Extract URLs** scans a block of text and pulls out every **http/https** URL it
contains — validated, deduplicated, and in first-seen order. Tick **Split into
components** to also break each URL into its **scheme**, **host**, **port**,
**path**, **query**, and **fragment**.

- **Validated**: candidates are parsed by a real URL parser, so malformed
  fragments are dropped.
- **Clean**: trailing prose punctuation (the period at the end of a sentence) is
  trimmed, and URLs wrapped in `( )` or `[ ]` come out without the brackets.
- **Deduplicated**: the same URL written twice counts once.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Collecting every link out of an email, document, or chat log.
- Auditing the query parameters / hosts referenced in a blob of text.
- Building a clean, unique link list from messy input.
