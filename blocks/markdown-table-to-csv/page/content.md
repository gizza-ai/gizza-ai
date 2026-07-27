## What this tool does

Paste a **Markdown table** (or any pipe-delimited table) and get back clean
**CSV**. It runs entirely in your browser — nothing is uploaded, it works
offline, and there's no sign-up.

The parser:

- drops the Markdown **alignment/separator row** (`---`, `:--`, `--:`, `:-:`),
- **trims cell padding** so `|  Ada   |` becomes `Ada`,
- treats the **outer pipes as optional** — `name | role` works as well as
  `| name | role |`,
- turns an escaped `\|` into a **literal pipe** inside a cell (and quotes that
  cell so it re-imports correctly),
- **quotes** any field that contains the delimiter, a quote, or a newline
  (RFC 4180),
- **pads ragged rows** out to the widest row, and
- **ignores surrounding prose** — you can paste a whole chat message and only the
  table lines are converted.

## Options

| Option | What it does |
| --- | --- |
| **Delimiter** | Output field separator. A single character, or a name: `comma` (default), `tab` (gives TSV), `semicolon`, `pipe`, `space`. |
| **First row is a header** | On by default — keeps the header row. Turn it off to drop the header and emit only the data rows. |
| **Quoting** | `minimal` (default) quotes a field only when it needs it; `all` wraps every field in double quotes. |
| **Line ending** | `lf` (default, `\n`, Unix/macOS) or `crlf` (`\r\n`, Windows/Excel). |
| **Add UTF-8 BOM** | Off by default. Turn it on so Excel opens the CSV as UTF-8 (keeps accents like `São` intact). |

## Examples

| Input (Markdown) | Settings | Output (CSV) |
| --- | --- | --- |
| `\| name \| role \|`<br>`\| --- \| --- \|`<br>`\| Ada \| Engineer \|` | default | `name,role`<br>`Ada,Engineer` |
| same as above | delimiter `tab` | `name` + TAB + `role` … (TSV) |
| `\| city \| note \|`<br>`\| --- \| --- \|`<br>`\| Paris, FR \| capital \|` | quoting `all` | `"city","note"`<br>`"Paris, FR","capital"` |
| `\| a \| b \|`<br>`\| --- \| --- \|`<br>`\| x \| y \|` | header off | `x,y` |

A cell with a comma is quoted automatically: `Paris, FR` becomes `"Paris, FR"`
so it stays one field.

## Limits & edge cases

- **One table at a time.** Every pipe-bearing line is treated as part of a single
  table — if you paste a document with several separate tables, they're merged.
  Convert one table per run.
- **Ragged rows are padded**, not rejected: a row with fewer cells than the widest
  row is filled with empty fields on the right.
- **Non-table lines are ignored** — any line without a `|` (prose, blank lines) is
  skipped. If nothing has a pipe, you get a clear "no table rows found" error.
- The output has **no trailing newline**; copy or download it as-is.

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes. The conversion runs locally in your browser with WebAssembly — your table
never leaves your device, and the page keeps working offline once it has loaded.
There's no account and no upload.

</details>

<details>
<summary>Does the input have to be a full Markdown table?</summary>

No. Outer pipes and the `---` alignment row are both optional. A plain
pipe-delimited block like `1 | 2 | 3` on each line converts fine — every non-blank
line that contains a `|` is treated as a row.

</details>

<details>
<summary>How do I get a tab-separated (TSV) file instead?</summary>

Set the **Delimiter** field to `tab` (or type a literal tab / `\t`). You can also
use `semicolon`, `pipe`, `space`, or any single character.

</details>

<details>
<summary>My cell contains a pipe character — how do I keep it?</summary>

Escape it in the Markdown as `\|`. The tool turns `\|` into a literal `|` inside
the cell instead of a column break, and quotes that field so it re-imports as one
value.

</details>

<details>
<summary>Excel shows my accents as garbage — what do I do?</summary>

Turn on **Add UTF-8 BOM**. The byte-order mark tells Excel the file is UTF-8, so
characters like `São` or `café` open correctly. For Windows imports you may also
want the **Line ending** set to `crlf`.

</details>

<details>
<summary>Can I drop the header row?</summary>

Yes — turn off **First row is a header**. The first row is removed and only the
data rows are written to the CSV.

</details>
