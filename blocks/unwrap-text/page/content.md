## About this tool

**Unwrap text** removes the **hard line breaks** that chop a paragraph into many
short lines — the kind you get when copying from an email, a PDF, a code comment,
or fixed-width output — and rejoins each paragraph into one continuous line.

- **Paragraph breaks are kept:** blank lines stay, so the structure survives.
- **Lists and quotes are protected:** lines starting with `-`, `*`, `+`, `>`, or
  `1.` stay on their own line (turn this off to join everything).
- Multiple blank lines collapse to a single paragraph break.

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat.

### Common uses

- Clean up text pasted from a PDF or terminal so it reflows in your editor.
- Un-wrap a quoted email before replying or re-formatting.
- Prepare hard-wrapped notes for a tool that expects one line per paragraph.

## FAQ

<details>
<summary>Which lines does the list/quote protection recognize?</summary>

Lines starting with `-`, `*`, `+` or `>` plus ordered-list markers in the
`1.` or `12)` style. With "keep list breaks" on (the default) those lines
keep their own line; switch it off and *every* line inside a paragraph is
joined, lists included.

</details>

<details>
<summary>How exactly are the lines rejoined?</summary>

Each paragraph — a run of consecutive non-blank lines — is joined with a
single space between lines. Words hyphenated across a line break are **not**
re-fused: `exam-` + `ple` becomes `exam- ple`, so fix end-of-line hyphens
before or after unwrapping PDF text.

</details>

<details>
<summary>Do blank lines survive?</summary>

Yes — a blank line always ends a paragraph and is preserved, so the document
structure survives. Runs of several blank lines are collapsed to a single
paragraph break rather than kept verbatim.

</details>
