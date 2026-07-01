## About this tool

**Sort Lines** puts the lines of any text in order — paste a list and choose how
it should be sorted.

- **Sort by** — *alpha* orders lines lexicographically (A→Z). *numeric* reads
  the leading number on each line and orders by value (so `2` comes before
  `10`). *natural* compares digit runs as numbers inside the text, so `file2`
  comes before `file10`. *length* orders by how long each line is.
- **Order** — *asc* (ascending) or *desc* (descending).
- **Ignore case** — treat upper- and lower-case as equal when comparing.
- **Ignore surrounding whitespace** — compare lines without their leading and
  trailing spaces.
- **Remove duplicate lines** — keep only the first occurrence of each line.
- **Remove blank lines** — discard empty or whitespace-only lines.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Alphabetising a list of names, words, imports or CSS properties.
- Ordering version or file names naturally (`v1`, `v2`, `v10`).
- Sorting numbers, scores or amounts that start each line.
- Tidying a list by removing duplicates and blank lines in one step.

## FAQ

<details>
<summary>What's the difference between numeric and natural sorting?</summary>

*Numeric* parses the number at the **start** of each line (signs and decimals
work, e.g. `-2.5`) and orders lines by that value. *Natural* compares digit runs
anywhere inside the text as numbers, so `file2` sorts before `file10` while the
letters around the digits are still compared as text.

</details>

<details>
<summary>Where do lines without a number go when I sort numerically?</summary>

Lines that don't begin with a number are placed after all the numbered lines
(and before them when the order is descending), rather than being dropped or
causing an error.

</details>

<details>
<summary>Does "Remove duplicate lines" respect the case and whitespace options?</summary>

Yes. Deduplication runs after sorting and uses the same comparison key, so with
**Ignore case** on, `Apple` and `apple` count as one line, and with **Ignore
surrounding whitespace** on, `  item` and `item` do too. The first occurrence is
kept and the output reports how many duplicates were removed.

</details>

<details>
<summary>Is there a size limit, and does my list leave my device?</summary>

There's no fixed line limit — sorting happens locally in your browser through
WebAssembly, so even large lists are processed on your machine and nothing is
uploaded.

</details>
