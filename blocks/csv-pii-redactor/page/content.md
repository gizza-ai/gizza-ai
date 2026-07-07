## About this tool

**CSV PII redactor** de-identifies the values in the CSV columns you choose — so you
can share a spreadsheet without leaking emails, phone numbers, card numbers, IDs, or
other personal data. Pick the columns (everything else passes through untouched) and
one of three deterministic **modes**:

- **Mask** — replace each character with a mask character, keeping the value's length.
  Set **keep last N** to leave a tail visible: a card `4111111111111111` with keep-last
  `4` becomes `************1111`.
- **Hash** — replace each value with a **salted SHA-256** hex code. It's deterministic,
  so the *same* value always maps to the *same* code — rows stay joinable and you can
  still `GROUP BY` — while the **salt** stops anyone reversing common values with a
  rainbow table. Choose how many hex characters to keep (8 for a short code, 64 for the
  full digest).
- **Redact** — replace every selected cell with a fixed label such as `[REDACTED]`.

### Worked example

Input CSV (redact the `email` column, mode **mask**):

```
name,email
Ada,ada@example.com
Bob,bob@corp.io
```

Output:

```
name,email
Ada,***************
Bob,***********
```

The `name` column and the header row are unchanged; only `email`'s data cells are masked.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded. Also
available from the [gizza CLI](/) and in chat.

### Columns by name or index

With **First row is a header** ticked, name columns directly (`email,card`). Untick it
to address columns by **1-based index** (`2,3`). Only the columns you list change; the
rest — and the header row — are copied through verbatim. Works with `,` / tab / `;` /
`|` delimiters (the same delimiter is used for reading and writing).

## FAQ

<details>
<summary>Is the hash reversible? What does the salt do?</summary>

No — SHA-256 is one-way, so a hashed value can't be turned back into the original. But
short, predictable values (a small set of IDs, common email addresses) can be
**guessed** by hashing every candidate and matching — a "rainbow table" attack. Adding a
**salt** that only you know defeats that: an attacker would have to know your salt to
build a matching table. The trade-off you keep is **linkability** — the same value plus
the same salt always yields the same code, so hashed columns still join across rows and
files. Use the same salt everywhere you want values to line up; change it to break the
link.

</details>

<details>
<summary>Can I keep part of a value visible, like the last 4 digits of a card?</summary>

Yes. In **mask** mode set **Keep last N** to the number of trailing characters to leave
readable. `4` turns `4111111111111111` into `************1111`. If N is greater than or
equal to the value's length, nothing is masked (the whole value stays visible), so keep
N small relative to your data.

</details>

<details>
<summary>My file has no header row — can I still pick columns?</summary>

Yes. Untick **First row is a header** and address columns by **1-based position**
instead: `2` is the second column. With a header present, each entry is matched against
the header names first and only treated as a number if no name matches. An index outside
the file's width (e.g. `9` in a 3-column file) is rejected with an out-of-range error.

</details>

<details>
<summary>How is this different from the free-text PII tools?</summary>

The [redact-pii](/tools/redact-pii/) and [pii-tokenize](/tools/pii-tokenize/) tools scan
**free prose** and detect PII by pattern (emails, phones, SSNs) anywhere in the text.
This tool is **column-scoped**: you say exactly which CSV columns to change and it never
touches the others, which is what you want for tabular exports. It also adds a
**salted-hash** mode that those text tools don't have. To simply drop columns instead of
redacting them, use [csv-reorder-columns](/tools/csv-reorder-columns/).

</details>

<details>
<summary>Can I apply a different mode to each column in one pass?</summary>

Not in a single run — one mode applies to every column you list. To mix modes (say,
hash the user ID but mask the card), run the tool once per group: first hash the ID
column, then paste the result back and mask the card column. Each pass only rewrites the
columns you name, so they compose cleanly.

</details>
