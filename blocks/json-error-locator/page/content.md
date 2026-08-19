## Find where your JSON breaks, not just that it broke

Parsers tell you *something* is wrong and stop at the first mistake. Paste the document above
and this tool tells you **where** — a 1-based line, a character-counted column, and a 0-based
offset for every syntax problem it finds — plus a plain-English cause, a concrete fix, and a
caret pointing at the exact character. Everything runs in your browser; the text is never
uploaded.

### What it names

- **Trailing commas** — `[1, 2, ]`, `{"a": 1,}`
- **Single-quoted strings and keys** — `{'a': 'b'}`
- **Unquoted keys** — `{name: "Ada"}` — and unquoted bare-word values
- **Missing commas** between members/elements, and a missing (or `=` instead of `:`) colon
- **Mismatched or unclosed brackets** — `{"a": 1]`, `{"a": {"b": 1}`
- **Unterminated strings**, pointing back at the opening quote that started them
- **Invalid escapes** — `"C:\Users"`, and `\u` escapes with fewer than four hex digits
- **Unescaped control characters** — a raw line break or tab inside a string
- **Invalid numbers** — `.5`, `1.`, `+1`, `007`, `0x1F`
- **JavaScript/Python literals** — `undefined`, `NaN`, `Infinity`, `True`, `None`
- **Comments** — `// line` and `/* block */`
- **Extra content** after the document's single top-level value

### Worked example

Input:

```
{
  "name": "Ada",
  "tags": [1, 2,]
}
```

Diagnosis (default settings):

```
Invalid JSON — 1 issue found.
A parser stops at line 3, column 17.

1. Line 3, column 16 (offset 34) — trailing comma
   Cause: this comma is the last thing in the array; JSON does not allow a comma before ].
   Fix:   delete this comma (or add another value after it before the ]).

   1 | {
   2 |   "name": "Ada",
   3 |   "tags": [1, 2,]
     |                ^
   4 | }
```

Switch **Result format** to machine-readable JSON and the same run returns
`{valid, issue_count, issues[], parser_stop, summary}` — one object per issue with `kind`,
`label`, `line`, `column`, `offset`, `explain`, `fix` and the snippet — ready to pipe into a
script or a CI check.

### Valid input still tells you something

A document that parses returns a summary instead of an error: the top-level value's type, how
many members or elements it holds, the nesting depth, and the line/byte count.

### Limits and edge cases

- **Input is capped at 1 MiB** and nesting at **200 levels**; deeper input reports
  `nesting too deep`, which in practice almost always means an unclosed bracket further up.
- **A position is where the problem was *detected*,** which can be just after the real
  mistake. For a missing comma or bracket, check the end of the previous line — the report
  repeats this note for exactly that reason.
- **Columns count characters, not bytes**, matching what your editor's cursor shows. `"héllo"`
  is 6 bytes but 5 characters wide.
- **An unterminated string ends the scan.** Everything after it was swallowed by the open
  quote, so any further finding would be an artefact rather than a real error.
- **Syntax only.** This tool diagnoses; it does not rewrite your JSON and does not check it
  against a JSON Schema.
- At most **50 issues** are listed — a badly mangled file cascades, and the first few are the
  ones worth fixing.

## FAQ

<details>
<summary>Why does the reported column point just after my mistake?</summary>

Because that is where the document first stops making sense. A missing comma is only
detectable at the *start of the next thing* — by the time `"b"` appears on line 3, the comma
that should have ended line 2 is already missing. The same applies to unclosed brackets: the
error surfaces at the end of the file, not where you forgot the `}`. The report flags the
detection point and reminds you to look at the line above; for unclosed containers it points
back at the opening bracket instead.

</details>

<details>
<summary>How is this different from a plain JSON validator?</summary>

A validator runs a real parser, which aborts at the first error — a file with five mistakes
takes five edit-and-retry rounds. This tool runs the parser *and* a tolerant scanner that
keeps going after each problem, so all five are listed at once with individual positions,
causes and fixes. Set **Report every issue** off to get the single-error, parser-style
behaviour instead.

</details>

<details>
<summary>My JSON came from JavaScript or Python and nothing parses it. What now?</summary>

That is the most common case here: single quotes, unquoted keys, `True`/`None`/`undefined`,
`NaN`, and `//` comments are all valid in those languages and none of them are JSON. Each one
is named individually with the exact replacement to make — for example `True` → `true`, or
`name:` → `"name":`. Fix them top-down and re-run; the issue list shrinks as you go.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The diagnosis is compiled to WebAssembly and runs entirely in your browser tab — no
request carries your JSON off the page, so pasting a config file or an API response with
credentials in it is safe. Closing the tab is all the cleanup there is.

</details>

<details>
<summary>What do the context-lines and offset controls do?</summary>

**Context lines** sets how many source lines are printed above and below each flagged line,
with a caret under the exact column — 2 by default, 0 to drop the snippets and get a compact
list of positions, causes and fixes. The **offset** in each heading is a 0-based character
offset from the start of the document, which is what `String.slice`, `seek`, and most editor
"go to position" commands want.

</details>

<details>
<summary>Can it fix the JSON for me?</summary>

Not here — this tool's job is the diagnosis, so you can see and understand every problem
before changing anything. Repairing malformed JSON, and re-indenting JSON that is already
valid, are separate tools; use the machine-readable output if you want to drive a fix from a
script.

</details>
