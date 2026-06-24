## What this tool does

Column Math does **element-wise arithmetic** between two columns of numbers,
right in your browser. Paste **Column A** and **Column B**, pick an operation,
and get the row-by-row result back. Nothing is sent to a server — it runs
locally, works offline, and needs no sign-up.

It is the quick way to do the kind of math a spreadsheet does with a fill-down
formula: sum two columns, take their difference, scale one by another, or compute
a ratio — without opening a spreadsheet.

## How it works

1. Paste your first list of numbers into **Column A** (one per line, or separated
   by commas or spaces).
2. Paste the matching list into **Column B** — it must have the **same number of
   values** as A.
3. Choose an **Operation** and read the result, one value per row.

## Operations

| Operation | What it computes (per row) |
| --- | --- |
| **add** (default) | A + B |
| **subtract** | A − B |
| **multiply** | A × B |
| **divide** | A ÷ B |

On **divide**, a zero in Column B is reported as an error (with the row number)
rather than producing infinity.

## Input format

- Values may be separated by **commas, spaces, or newlines** — or any mix of them.
- Blank lines and trailing separators are ignored, so a trailing comma or an
  empty last line is fine.
- Decimals and negative numbers are supported. Whole-number results print without
  a trailing `.0` (so `2 + 2` is `4`, not `4.0`).

## Examples

| Column A | Column B | Operation | Result |
| --- | --- | --- | --- |
| `1, 2, 3` | `4, 5, 6` | add | `5, 7, 9` |
| `10, 20` | `3, 5` | subtract | `7, 15` |
| `2, 3` | `4, 5` | multiply | `8, 15` |
| `10, 9` | `2, 3` | divide | `5, 3` |
| `1.5, 2.25` | `0.5, 0.25` | add | `2, 2.5` |

## FAQ

**Is it free and private?** Yes — your numbers never leave your device, and the
tool keeps working offline once the page has loaded.

**What if the two columns are different lengths?** You will get an error telling
you how many values each column has, so you can line them up.

**Can I paste a column straight from a spreadsheet?** Yes — a spreadsheet column
copies as one value per line, which is exactly the format this tool accepts.

**Does it handle decimals and negatives?** Yes. Mix integers, decimals, and
negative numbers freely in either column.
