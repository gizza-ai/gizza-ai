## About this tool

**Regex Tester** runs a regular expression against a block of text and gives you
a full, structured breakdown of what it matched — the way a regex debugger does.
For **every match** it reports the character start/end position, and within each
match it lists the value and span of **every capture group**, both numbered and
named.

- **Match positions**: each match is reported with its 0-based character offsets,
  so you can see exactly where in the text it landed (Unicode-aware — positions
  count characters, not bytes).
- **Capture groups**: numbered groups `(…)` and named groups `(?<name>…)` are
  both broken out, with their own value and span. Optional groups that did not
  participate are shown as *(no match)* rather than silently dropped.
- **Match flags**: tick **Ignore case** for case-insensitive matching,
  **Multiline** so `^` and `$` anchor at the start and end of each line, and
  **Dot matches newline** so `.` also spans line breaks.

The syntax is the [Rust `regex`](https://docs.rs/regex/) flavour — a clean,
linear-time engine with no catastrophic backtracking, so even pathological
patterns stay fast.

Everything runs **locally in your browser** via WebAssembly — your text and your
pattern are never uploaded.

### Handy for

- Debugging why a pattern matches more or less than you expected.
- Checking that named and numbered capture groups grab the right sub-strings.
- Confirming match positions before wiring a regex into code.

### Looking to pull matches out instead?

If you just want the list of matches (or a single capture group) rather than the
full positional breakdown, use the companion [Regex Extract](/tools/regex-extract/)
tool.
