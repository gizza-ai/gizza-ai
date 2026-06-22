## About this tool

**Frequency distribution** counts how often each symbol appears in your input and
ranks the symbols from **most to least frequent**, with both a raw **count** and a
**percentage** of the total. It's the building block of classic frequency analysis.

Choose what to tally:

- **Characters** (default): each Unicode character. Spaces, tabs and newlines are
  shown with readable labels (`␠ (space)`, `\t (tab)`, `\n (newline)`).
- **Bytes**: the raw bytes (shown as `0xNN`). A character like `é` is two UTF-8
  bytes, so byte and character counts differ for non-ASCII text.
- **N-grams**: overlapping windows of *n* characters (set **N-gram size** — 2 gives
  bigrams, 3 trigrams, and so on).

You can read the input as plain **text** or as a **hex** string (whitespace and an
optional `0x` prefix are ignored) — handy for analyzing binary or encoded data.

### Privacy

Everything runs **in your browser** via WebAssembly — your input is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat
(which return the distribution as structured JSON with counts and percentages).

### Common uses

- Classic **letter-frequency analysis** for cryptograms and substitution ciphers.
- Inspect the **byte histogram** of a file or payload (paste it as hex).
- Find the most common **bigrams / trigrams** in a body of text.
- Spot skew or padding characters in encoded data.
