## About this tool

**Remove Empty Lines** deletes the blank lines from a block of text so the
remaining lines sit together with no gaps. Paste your text, pick how blank lines
should be handled, and copy the compacted result.

- **What to do with blank lines**
  - *Remove all empty lines* — delete every blank line, leaving no gaps between
    content (the default).
  - *Collapse consecutive blanks to one* — reduce any run of two or more blank
    lines down to a single blank line, so paragraphs stay separated without
    large gaps.
- **Also remove whitespace-only lines** — treat a line that contains only spaces
  or tabs (or any other whitespace) as empty, not just a line that is literally
  empty. On by default.
- **Trim leading/trailing whitespace on kept lines** — also strip spaces and
  tabs from the start and end of the lines that remain. Off by default so
  indentation is preserved unless you ask for it.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Worked example

Input:

```
First line

Second line
   
Third line

```

With *Remove all empty lines* and *Also remove whitespace-only lines* on (the
line with three spaces counts as empty), the output is:

```
First line
Second line
Third line
```

Switch the mode to *Collapse consecutive blanks to one* and the same input keeps
one blank line between each entry instead of removing them all.

### Handy for

- Cleaning up text pasted from a PDF or email that arrived double-spaced.
- Compacting a list where blank lines crept in between items.
- Tidying code or config after deletions left empty gaps behind.

### Limits & edge cases

- Line endings are normalized to `\n` (LF): Windows `\r\n` and old-Mac `\r`
  inputs come out with plain LF line breaks.
- A trailing newline at the very end counts as a final empty line and is removed,
  so the result never ends with a dangling blank line in *remove* mode.
- "Whitespace" follows Unicode rules, so non-breaking spaces (U+00A0) and the
  ideographic space (U+3000) also make a line count as empty when
  *whitespace-only* is on.
- This tool only removes blank lines. To collapse runs of spaces inside a line,
  or to strip every space, use a whitespace-cleaning tool instead.

### FAQ

<details>
<summary>What counts as an "empty" line?</summary>

By default, both a truly empty line and a line that contains only spaces or tabs
count as empty and are removed. If you turn off **Also remove whitespace-only
lines**, then only lines with zero characters are removed and a line of spaces is
kept as-is.

</details>

<details>
<summary>What is the difference between "remove all" and "collapse"?</summary>

**Remove all** deletes every blank line, so the remaining content is packed
together with no gaps. **Collapse** keeps a single blank line wherever there were
one or more in a row — useful when you want to keep paragraph breaks but get rid
of triple- and quadruple-spaced gaps.

</details>

<details>
<summary>Will it change the non-blank lines or their order?</summary>

No. The order of your content is preserved exactly. Non-blank lines are left
untouched unless you tick **Trim leading/trailing whitespace on kept lines**,
which strips spaces and tabs from the start and end of each remaining line.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The tool runs entirely in your browser as WebAssembly — the text you paste
never leaves your device.

</details>
