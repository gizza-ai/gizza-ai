## About this tool

**CSV reorder columns** rearranges a CSV's columns to the exact order you want —
and drops the ones you leave out.

Give a **target order** as a comma-separated list of **column names** (when the
first row is a header) or **1-based indices**:

- `city,name` → keep only those two, in that order
- `3,1,2` → reorder by position
- repeat a name to **duplicate** a column

Columns you don't list are **dropped**. Works with `,` / tab / `;` / `|`
delimiters.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat.

### Common uses

- Move the most important columns to the front.
- Drop columns you don't need before sharing a CSV.
- Swap two columns, or reorder to match another file's schema.

## FAQ

<details>
<summary>How do I drop columns I don't want?</summary>

Just leave them out of the target order. Only the columns you list are kept — so
with a `name,age,city` file, entering `name,city` returns those two columns and
silently drops `age`. There's no separate "delete column" option; the list *is*
the output schema.

</details>

<details>
<summary>My CSV has no header row — can I still reorder it?</summary>

Yes. Untick the header option and address columns by **1-based position** instead:
`3,1,2` puts the third column first. An index outside the file's width (e.g. `9`
in a 2-column file) is rejected with an out-of-range error rather than producing
empty columns. When a header *is* present, each entry is matched against the
header names first and only treated as a number if no name matches.

</details>

<details>
<summary>Can I duplicate a column?</summary>

Yes — repeat its name or index in the list. `name,city,name` outputs the `name`
column twice, which is handy when two downstream systems expect the same value
under different positions.

</details>

<details>
<summary>Does it only work with commas?</summary>

No. The delimiter option accepts `comma` (default), `tab`, `semicolon`, `pipe`, or
any other single character. The same delimiter is used for reading and writing, so
a tab-separated file stays tab-separated.

</details>
