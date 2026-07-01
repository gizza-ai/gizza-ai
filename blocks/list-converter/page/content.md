## Reformat and Clean Lists Offline

Convert, sort, deduplicate, and transform lists instantly in your browser. This tool handles text columns, SQL arrays, JSON, XML, custom separators, prefixing/suffixing, and text casing in a single pass. Everything runs locally; your data never leaves your device.

---

### Features & Capabilities

- **Bidirectional & Multi-Format Splitting**: Auto-detects input separators (newline, comma, semicolon, pipe, tab) or allows splitting by a custom string delimiter.
- **Rich Output Layouts**:
  - **Comma / CSV**: `apple, banana, cherry`
  - **Newline**: One item per line
  - **Tab / Pipe**: Tab-separated or pipe-separated (`|`) lists
  - **JSON Array**: `["apple", "banana", "cherry"]`
  - **SQL IN Clause**: `('apple', 'banana', 'cherry')` (perfect for SQL queries, escaping single quotes automatically)
  - **XML Elements**: `<item>apple</item>` (useful for markup copying)
  - **Bulleted / Numbered**: `- apple` or `1. apple`
  - **Quoted / Custom**: Wrap items in double quotes or join them using any custom string.
- **Advanced Sorting**: Sort alphabetically (Ascending/Descending), by text length (Shortest/Longest first), or randomize/shuffle the list order.
- **Bulk Transformations**: Prepend prefixes, append suffixes, or transform text casing (lowercase, uppercase, title case) for all items simultaneously.

---

### Frequently Asked Questions

<details>
<summary>Where is my list data sent?</summary>

Your data never leaves your computer. The splitting, cleaning, and formatting logic runs entirely inside your browser using WebAssembly. There are no backend database calls, analytical trackers, or server-side logging.

</details>

<details>
<summary>How does the SQL IN layout handle quotes?</summary>

The SQL output format wraps each list item in single quotes (`'item'`) and automatically escapes any internal single quotes as double single-quotes (`''`), which is standard SQL syntax. It then wraps the final list in parentheses `(...)`.

</details>

<details>
<summary>What does the 'Auto' input separator do?</summary>

'Auto' scans the input list and splits by the first matching priority separator: first looking for newlines, then commas, then semicolons, pipes, and tabs. If none are found, it falls back to parsing as a single-item list.

</details>

<details>
<summary>How does the Deduplicate feature work?</summary>

Deduplication is case-sensitive and preserves the first occurrence of each unique item in the list, removing any subsequent duplicates without altering the remaining order of your list.

</details>
