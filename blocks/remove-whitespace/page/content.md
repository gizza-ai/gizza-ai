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
