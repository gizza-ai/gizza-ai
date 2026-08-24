## About this tool

A Markdown file that three people have edited ends up with three bullet styles. Someone types `*`, someone else `-`, a paste from a doc brings in `+`, one sublist is indented three spaces and the next one uses a tab, and the numbered list restarts at 3. It all renders fine — and it makes every diff noisier than the change it contains.

This tool rewrites the *scaffolding* of your lists and nothing else. Paste the document, choose a bullet character and an indentation step, and every marker, every nesting level, and every ordered number comes out uniform. Item text is never touched.

A worked example. With the defaults — dash bullets, 2 spaces per level, indentation fixed, numbering kept, one space after each marker:

```
* Setup
   + Install the CLI
   + Configure the token
- Usage
	* Run a build
```

becomes

```
- Setup
  - Install the CLI
  - Configure the token
- Usage
  - Run a build
```

Three different markers became one. The 3-space sublist and the tab-indented item both landed on the same 2-space ladder. `Install the CLI` still reads exactly as it did.

**Bullet marker** picks the character: `-`, `*`, `+`, **Consistent** (reuse whichever marker the file used first, so an already-uniform document is left alone), or **Sublist** (`-` at the top level, `*` one level in, `+` two levels in, then repeating — useful when you want the nesting depth visible in the raw text). Dash is the default and the style most formatters and house style guides emit.

**Spaces per nesting level** is the indentation step, 1 to 8. Two matches CommonMark and the common formatter default; three keeps ordered sublists safe under Kramdown; four lines nesting up with code indentation. Sloppy input is snapped to the nearest rung — an item indented 1, 3, or 5 spaces becomes a clean child or a clean sibling, and tabs are expanded at 4 columns then written back as spaces. A line has to be at least 2 columns deeper than its parent item to count as a new level; anything less is read as a sibling that drifted.

**Ordered list numbering** handles the numbers: **Keep** leaves them as written, **Sequential** renumbers each list from its own first number (`3. 7. 9.` → `3. 4. 5.`), **All ones** and **All zeros** write every item as `1.` or `0.` so reordering steps produces a one-line diff instead of renumbering the whole list. Nested ordered lists are counted independently, and the `.` or `)` after the number is always preserved.

**Fix indentation steps** can be turned off to change only markers, numbering, and spacing while leaving every line's original leading whitespace exactly as it is. **Spaces after the marker** sets the gap between the marker and the text, 1 to 4.

Limits and edge cases:

- Up to 500,000 characters per run. Everything happens in your browser — the document is never uploaded, so drafts and internal notes are safe to paste.
- Fenced code blocks are copied through verbatim. A ``` or `~~~` block containing `* not a bullet` stays exactly that, indentation included.
- Thematic breaks (`---`, `***`, `___`) are recognised as horizontal rules, not as bullets, and line-start emphasis such as `*emphasis*` is left alone — a bullet needs whitespace after the marker.
- Task-list checkboxes ride along: `* [ ] todo` becomes `- [ ] todo`, with the box untouched.
- A wrapped continuation line under an item is shifted by the same amount its marker moved, so it stays attached to its item.
- Blank lines never close a list, so loose lists keep their spacing. Unindented prose, a heading, or a table does close it, and the next list starts its nesting from scratch.
- Headings, tables, blockquotes, link text, HTML, and front matter are passed through untouched. CRLF line endings survive, and a file with no trailing newline gets none added.
- Nothing is rendered or validated. This is a text-in, text-out rewrite — it will not tell you whether your Markdown is otherwise correct.

## FAQ

<details>
<summary>Which bullet character should I standardise on?</summary>

Any of the three is valid CommonMark, so pick one and stay consistent. `-` is the safe default: it is what most formatters emit, it reads cleanly in raw text, and it never collides with emphasis. `*` matches the style some older toolchains and the unified/remark ecosystem default to. `+` is rare enough that it stands out, which is exactly why some people use it for a specific nesting level. If your repository already has a linter rule set, match its `ul-style` value — the five options here use that same vocabulary.

</details>

<details>
<summary>Why did my 3-space sublist become a 2-space one?</summary>

Because the indentation is snapped onto an exact ladder. Nesting *depth* is preserved — what changes is how many spaces each level costs. An item that was one level deep at 3 spaces comes back one level deep at whatever you set "Spaces per nesting level" to. If a line was indented too little to be a real child (less than 2 columns deeper than the item above it), it is treated as a sibling that drifted and pulled back to its parent's column. To keep your original columns and only change the marker characters, turn off "Fix indentation steps".

</details>

<details>
<summary>Will it touch anything inside a code block?</summary>

No. Fenced blocks opened with ``` or `~~~` are tracked and every line inside them is copied through byte-for-byte, including lines that look exactly like list items. This matters for shell snippets, YAML, and diff output, where a leading `-` or `*` is meaningful and rewriting it would break the example. Indented (four-space) code blocks are a known limitation: they are indistinguishable from deeply nested list content in a line-based pass, so put your examples in fenced blocks if they contain bullet-like lines.

</details>

<details>
<summary>What is the difference between "All ones" and "Sequential" numbering?</summary>

Both produce a correctly rendered numbered list — Markdown renderers count items themselves and ignore the numbers you write, apart from the first one. **Sequential** writes the real running count, which reads better in raw text and matches what a reader sees. **All ones** writes `1.` on every item, which means inserting a step in the middle changes one line in the diff instead of renumbering everything below it. Teams that review Markdown in pull requests usually prefer all-ones; documents meant to be read as plain text usually prefer sequential.

</details>

<details>
<summary>Can it renumber a nested ordered list correctly?</summary>

Yes. Each nesting level keeps its own counter, so an outer `1. 2.` and an inner `1. 2. 3.` are numbered independently, and the inner list restarting does not disturb the outer one. Each list is seeded from its own first number, so a list that deliberately starts at `5.` continues `6. 7.` rather than being reset to 1. The `)` delimiter is kept if you used it — `1)` stays `1)`, because changing it to `1.` can make some renderers treat the result as a brand-new list.

</details>

<details>
<summary>Does it check my Markdown for other problems?</summary>

No — this tool changes list scaffolding only. It will not fix heading spacing, strip trailing whitespace, collapse repeated blank lines, add the missing newline at the end of the file, or reformat tables. That narrowness is intentional: it means you can run it on a document and be certain the only lines in the diff are list lines. For the broader style pass, use a general Markdown linter alongside it.

</details>
