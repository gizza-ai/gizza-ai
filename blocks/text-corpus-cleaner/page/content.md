## Clean a raw text corpus line by line

Paste a line-oriented corpus — one record, sentence, or word per line — and this
tool cleans every line in one deterministic pass, locally in your browser.
**Nothing is uploaded.** The steps always run in the same fixed order:

1. **Split** on line endings (`\n`, `\r\n`, or `\r`).
2. **Transform** each line: Unicode normalization → whitespace handling →
   optional lowercase.
3. **Filter** out lines that are too short (**min characters** / **min words**),
   mostly punctuation (**max symbol ratio**), or not the **target language**.
4. **Deduplicate** non-blank lines, keeping the first occurrence.
5. **Blank-line handling**: optionally collapse runs of blank lines to a single
   blank and trim leading/trailing blanks (paragraph breaks are preserved).

Language detection is trigram/script based (no model files, no network), so lines
too short to identify are always kept rather than silently dropped.

### Worked example

With **Whitespace → Collapse** and **Deduplicate → Normalized**, this input:

```
The Quick  Brown Fox
the quick brown fox
The Quick  Brown Fox
```

becomes:

```
The Quick Brown Fox
```

Collapsing squeezes the double space in line 1 to a single space. Normalized
dedupe compares lines after folding case and interior spacing, so all three lines
share the key `the quick brown fox` — only the **first occurrence is kept, verbatim
after its transforms** (`The Quick Brown Fox`), and the two repeats are dropped.

### Limits & edge cases

- **Line-oriented only.** Input is split on newlines and each line is cleaned
  independently — this tool does not reflow paragraphs or wrap text.
- **Blank lines are structural.** They are never language- or symbol-filtered and
  never deduplicated, so paragraph breaks survive. A positive **min characters**
  or **min words** does drop blank lines (they have 0 of each).
- **Symbol ratio** counts non-alphanumeric, non-space characters against visible
  characters; `1.0` keeps everything, `0.5` drops lines that are more than half
  punctuation. A line with no visible characters has a ratio of `0`.
- **No trailing newline** is added — cleaned lines are joined with single `\n`s.
- **Detection needs enough text.** Very short lines can't be language-detected and
  are kept even when a target language is set, so mixed-language word lists may
  keep short foreign words.
- Everything runs in memory, so very large corpora are bounded by your browser's
  available memory.

### FAQ

<details>
<summary>What is the difference between exact and normalized deduplication?</summary>

**Exact** drops a line only when it is byte-for-byte identical to an earlier line.
**Normalized** folds case and collapses interior whitespace before comparing, so
`Hello   World` and `hello world` count as duplicates. Either way the **first**
occurrence is kept exactly as it appears after the other transforms; only later
repeats are removed. Blank lines are never treated as duplicates.

</details>

<details>
<summary>When should I use NFC versus NFKC normalization?</summary>

Use **NFC** (the recommended default) for everyday text — it composes characters
into their canonical form without changing what they mean. Use **NFKC** when you
want to flatten *compatibility* variants: it turns ligatures like `ﬁ` into `fi`,
full-width `ＡＢＣ` into `ABC`, circled or styled letters into plain ones, and
fractions into their spelled-out forms. Pick **None** to leave code points exactly
as they are.

</details>

<details>
<summary>How does the language filter decide what to keep?</summary>

Set a target language and each non-blank line is passed through trigram/script
based detection (the `whatlang` approach — no downloaded model, no network call).
A line is kept when its detected language matches the target. A line that is **too
short or too ambiguous to detect is kept**, not dropped, so short valid records
aren't lost. Leave the filter on **Any** to disable it entirely.

</details>

<details>
<summary>Why are some short or symbol-heavy lines still in the output?</summary>

The length and symbol filters only apply when you set them. **Min characters** and
**min words** default to `0` (keep everything), and **max symbol ratio** defaults
to `1.0` (keep everything). Raise the minimums or lower the ratio to trim junk —
for example `min words = 3` and `max symbol ratio = 0.5` removes one- or two-word
fragments and lines that are mostly `=====` or `>>>>>` separators.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The whole clean runs locally with WebAssembly; your corpus never leaves your
browser, so it is safe to paste private or proprietary text.

</details>
