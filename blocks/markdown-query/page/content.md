## About this tool

Markdown Query is a small "jq for Markdown": paste a Markdown document, choose
what you want, and get just those pieces back. Pick an element type — **headings**,
**links**, **images**, **code blocks**, or **tables** — and the tool parses the
document and pulls out every match. It understands standard CommonMark plus
GitHub-flavored pipe tables, so inline and reference links, autolinks, fenced code
with a language tag, and multi-column tables all come through correctly.

Choose how the results come out: **Text** for a quick human-readable list, **JSON**
for a structured `{ count, items }` payload you can feed to a script, or **Markdown**
to get clean, reconstructed snippets you can paste elsewhere. Turn on **source line
numbers** to see exactly where each item lives in the original document — handy for
linting docs, auditing links, or building a table of contents.

Everything runs locally in your browser through a WebAssembly parser. Your Markdown
is never uploaded, there is no sign-up, and the output is fully deterministic — the
same input always produces the same result.

## FAQ

<details>
<summary>What can I extract from my Markdown?</summary>

Five element types: **headings** (with their level), **links** (text plus the
destination URL and any title), **images** (alt text plus the source URL),
**code blocks** (the code plus its language, if the fence has one), and **tables**
(the header row plus every body row). Pick one at a time from the *Extract* control.

</details>

<details>
<summary>What is the difference between the Text, JSON, and Markdown formats?</summary>

**Text** gives a compact, human-readable listing — one item per line, with code and
tables shown as blocks. **JSON** returns a structured object with a `count` and an
array of items, ideal for scripting. **Markdown** rebuilds each match as valid
Markdown (for example `[text](url)` for links or a fenced block for code) so you can
paste it straight into another document.

</details>

<details>
<summary>Does it handle tables and fenced code blocks?</summary>

Yes. Tables use GitHub-flavored pipe syntax, and column alignment is preserved when
you export back to Markdown. Fenced code blocks keep their language tag (` ```rust `),
and indented code blocks are captured too — they just report an empty language.

</details>

<details>
<summary>What do the source line numbers show?</summary>

When **Include source line numbers** is on, each item is annotated with the 1-based
line in your original Markdown where it begins. In JSON output this appears as a
`line` field; in text and Markdown output it is shown as a small prefix or comment.
It is useful for linting, auditing links, or jumping back to the source.

</details>

<details>
<summary>Is my Markdown uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs entirely in your browser. Nothing
is sent to a server, there is no sign-up, and the same input always yields the same
output.

</details>
