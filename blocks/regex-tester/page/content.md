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

## FAQ

<details>
<summary>Why is my lookahead or backreference rejected?</summary>

The engine is the Rust `regex` flavour, which deliberately has **no
lookahead/lookbehind and no backreferences** — that's what guarantees
linear-time matching with no catastrophic backtracking. Patterns using
`(?=…)`, `(?!…)` or `\1` fail with a syntax error; usually you can
restructure with alternation or capture groups instead.

</details>

<details>
<summary>Are the match positions byte offsets or character offsets?</summary>

0-based **character** offsets, Unicode-aware — `é` or a CJK character counts
as one position, not two or three bytes. That means the numbers line up with
what you'd get from JavaScript's `String.prototype.slice` on most text, not
with raw UTF-8 byte indices.

</details>

<details>
<summary>What does “(no match)” next to a capture group mean?</summary>

The group is optional (e.g. `(\d+)?` or one arm of an alternation) and did
not participate in that particular match. It's reported explicitly instead of
being silently dropped, so group numbering stays stable across matches.

</details>

<details>
<summary>How do the three flag checkboxes map to regex flags?</summary>

**Ignore case** is `i`; **Multiline** is `m`, making `^`/`$` anchor at every
line instead of just the whole text; **Dot matches newline** is `s`, letting
`.` cross line breaks. They can be combined freely, and an empty pattern is
rejected rather than matching everywhere.

</details>
