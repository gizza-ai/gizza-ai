## About this tool

**JSON Beautifier** reformats minified or messy JSON into clean, readable text —
with the indentation you choose — and **validates** it along the way.

- **Indent** sets the spaces per level (1–8). Set **indent to 0** to *minify*
  instead: the whole document collapses to one compact line.
- **Validation built in**: if the JSON is malformed, you get the parser's exact
  message (with line/column) instead of broken output.
- **Key order is preserved** — objects come out in the same order you wrote them,
  not alphabetically sorted.

Everything runs **locally in your browser** via WebAssembly — your data is never
uploaded.

### Handy for

- Making an API response or config readable.
- Minifying JSON before embedding it somewhere size-sensitive.
- Quickly checking whether a blob is valid JSON and where it breaks.

## FAQ

<details>
<summary>How do I minify instead of pretty-print?</summary>

Set the indent to **0**. The document is still parsed and validated, then
re-serialized as one compact line with all insignificant whitespace removed —
ideal before embedding JSON in a URL, env var, or HTTP header.

</details>

<details>
<summary>Will my object keys get re-sorted?</summary>

No. Key order is preserved exactly as written — `{"b":1,"a":2}` stays `b`
before `a` after formatting. Only whitespace changes, so diffs against the
original stay meaningful.

</details>

<details>
<summary>Why does it reject JSON with trailing commas or comments?</summary>

Because they aren't valid JSON — the tool is also a validator, so it parses
strictly and reports the parser's exact message with the line and column of
the first problem (e.g. `[1,2,]` fails on the trailing comma). JSON5/JSONC
extensions like comments or unquoted keys are reported as errors too.

</details>

<details>
<summary>What indentation sizes are supported?</summary>

Anything from 1 to 8 spaces per level (default 2); larger values are clamped
to 8, and 0 switches to minify mode. Tabs aren't offered — the indent is
always spaces.

</details>
