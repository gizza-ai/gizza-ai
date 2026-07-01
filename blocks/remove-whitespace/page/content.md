## About this tool

**Remove whitespace** cleans up messy spacing in text. Pick a mode:

- **Trim** (default): removes leading and trailing whitespace from every line,
  plus any blank lines at the very start or end. Internal spacing and line
  structure are kept.
- **Collapse**: trims each line *and* squashes runs of spaces or tabs inside a
  line down to a single space — great for fixing double spaces or stray tabs.
- **Strip**: removes **every** whitespace character, including Unicode spaces
  like the non-breaking space (NBSP), leaving one dense string.

Turn on **Collapse blank lines** to squash runs of two or more blank lines down
to a single blank line (Trim and Collapse modes only).

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat.

### Common uses

- Clean up text pasted from a PDF, email, or terminal that has ragged spacing.
- Remove double spaces and stray tabs before pasting into a document.
- Strip all whitespace to build a compact token, slug, or fingerprint.

## FAQ

<details>
<summary>Which mode fixes double spaces without joining my lines?</summary>

**Collapse.** It squashes runs of spaces or tabs *inside* a line down to one
space while keeping your line breaks, and it trims each line's edges too.
**Trim** only cleans line edges (double spaces inside a line survive), and
**Strip** goes further than you want — it removes newlines as well.

</details>

<details>
<summary>Does it catch non-breaking spaces pasted from Word or a PDF?</summary>

Yes. "Whitespace" here follows Unicode, not just ASCII — so NBSP (U+00A0),
the ideographic space (U+3000), and similar characters are recognized.
Collapse turns an NBSP run into a single regular space, and Strip removes
them entirely, which is exactly what invisible-character cleanup needs.

</details>

<details>
<summary>Will Trim mode remove my code indentation?</summary>

It will — Trim strips leading whitespace from **every** line, so indented
code or nested lists come out flush-left. Spacing *within* each line is
untouched. If you only want to drop blank lines around the text, that's still
Trim, just be aware indentation goes with it.

</details>

<details>
<summary>Why does "Collapse blank lines" have no effect in Strip mode?</summary>

Strip removes every whitespace character, newlines included, so there are no
blank lines left to collapse — the option is ignored there. It applies in
Trim and Collapse modes, where it squashes runs of two or more blank lines
down to one.

</details>
