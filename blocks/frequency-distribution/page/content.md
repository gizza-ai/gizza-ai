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

## FAQ

<details>
<summary>Can I count 'A' and 'a' as the same letter?</summary>

Yes — turn **case sensitive** off and characters and n-grams are folded to
lowercase before tallying, so `The` and `the` land in the same bucket. Byte
mode ignores the switch: raw bytes have no case, so `0x41` and `0x61` always
stay separate there.

</details>

<details>
<summary>Do the n-grams overlap?</summary>

Yes. N-gram mode slides a window one character at a time, so `banana` with
size 2 yields `ba`, `an`, `na`, `an`, `na` — `an` and `na` each count twice.
The size must be at least 1; size 2 gives the classic bigram table used in
cipher analysis.

</details>

<details>
<summary>How do I analyze binary data instead of text?</summary>

Set the input kind to **hex** and paste the bytes as a hex string — whitespace
and a leading `0x` are ignored. An odd number of hex digits or a non-hex
character is reported as an error rather than silently skipped, so you know
the paste was clean.

</details>

<details>
<summary>Why don't byte counts match character counts for my text?</summary>

UTF-8. ASCII letters are one byte each, but `é` is two bytes and most emoji
are four, so byte mode splits one character across several `0xNN` entries.
Use **char** mode for human-readable text and **byte** mode when you care
about the raw encoding.

</details>
