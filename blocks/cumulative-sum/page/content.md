## What this tool does

The Cumulative Sum Calculator produces the **running total** — the
**prefix sum** — of a list of numbers, right in your browser. Paste your
numbers, and get back a table with one row per value: the value itself and the
cumulative sum up to that point. Nothing is sent to a server — it runs locally,
works offline, and needs no sign-up.

It is the quick way to turn a column of numbers into a running balance or a
step-by-step accumulation, the way a spreadsheet does with a `=SUM($A$1:A1)`
fill-down formula — without opening a spreadsheet.

## How it works

1. Paste your list of numbers into **Numbers** (one per line, or separated by
   commas or spaces).
2. Optionally tick **Running average**, **Running minimum**, and/or **Running
   maximum** to add those columns.
3. Read the result — one row per input value, columns separated by ` | `.

## Columns

| Column | What it shows (at each row) |
| --- | --- |
| **value** | the input number for that row |
| **cumulative_sum** | the running total of every value up to and including this one |
| **running_avg** *(optional)* | the average (mean) of the values seen so far |
| **running_min** *(optional)* | the smallest value seen so far |
| **running_max** *(optional)* | the largest value seen so far |

## Input format

- Values may be separated by **commas, spaces, or newlines** — or any mix of them.
- Blank lines and trailing separators are ignored, so a trailing comma or an
  empty last line is fine.
- Decimals and negative numbers are supported. Whole-number results print without
  a trailing `.0` (so `2 + 2` shows as `4`, not `4.0`).

## Examples

A plain running total of `1, 2, 3, 4`:

```
value | cumulative_sum
1 | 1
2 | 3
3 | 6
4 | 10
```

With the running average, minimum, and maximum turned on for `3, 1, 4, 1, 5`:

```
value | cumulative_sum | running_avg | running_min | running_max
3 | 3 | 3 | 3 | 3
1 | 4 | 2 | 1 | 3
4 | 8 | 2.6666666667 | 1 | 4
1 | 9 | 2.25 | 1 | 4
5 | 14 | 2.8 | 1 | 5
```

## FAQ

**What is a cumulative sum?** It is the running total: each entry is the sum of
all the values up to and including that position. The last value of the
cumulative sum equals the grand total of the whole list.

**Is it free and private?** Yes — your numbers never leave your device, and the
tool keeps working offline once the page has loaded.

**Can I paste a column straight from a spreadsheet?** Yes — a spreadsheet column
copies as one value per line, which is exactly the format this tool accepts.

**Does it handle decimals and negatives?** Yes. Mix integers, decimals, and
negative numbers freely. Negative values simply make the running total go down.

**What's the difference between the running average and a plain average?** The
running average at each row is the mean of just the values seen *so far*, so it
changes row by row; the plain average is a single number for the whole list (the
running average on the final row).
