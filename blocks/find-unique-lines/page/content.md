## About this tool

**Find Unique Lines** scans pasted text and keeps only the lines that appear
**exactly once** — every line that repeats is dropped entirely. This is the
classic `uniq -u` behaviour: not deduplication (which keeps one copy of each
line), but *one-off detection*.

- **Ignore case** — treat `Foo`, `foo`, and `FOO` as the same line, so a value
  that recurs in mixed case is correctly excluded.
- **Trim whitespace** — ignore leading/trailing spaces and tabs when comparing,
  so `  item` and `item` count as the same line.

Unique lines are returned in the order they first appeared. The tool also
reports the total number of lines and the number of distinct lines.
Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Finding values that occur in one list but not another (paste both, keep the
  one-offs).
- Spotting outliers or typos that appear only once in an export or log.
- Isolating rows that didn't get duplicated by a faulty merge or join.

> Want to *count* how often each line repeats, or list the duplicates instead?
> Use a duplicate-finder tool; this one keeps **only the lines that appear
> once**.

## FAQ

<details>
<summary>How is this different from removing duplicates?</summary>

Deduplication keeps one copy of every line; this tool (like `uniq -u`) keeps
only lines that occur **exactly once** and drops every repeated line entirely.
If a value appears twice, it won't be in the output at all.

</details>

<details>
<summary>Is the comparison case-sensitive?</summary>

By default, yes — `Foo` and `foo` are different lines, so if each appears once
they are both kept. Turn on **Ignore case** to treat them as the same line, in
which case the pair counts as a repeat and both are dropped.

</details>

<details>
<summary>A line looks unique in my text but was dropped — why?</summary>

Check the normalization options: with **Trim whitespace** on, `  item` and
`item` compare equal, and with **Ignore case** on, `NYC` and `nyc` do too. A
line that repeats only under normalization is excluded as a duplicate.

</details>

<details>
<summary>What order do the results come out in?</summary>

The order the unique lines first appeared in your input — nothing is sorted.
The result also reports the total line count and the number of distinct lines.

</details>
