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
