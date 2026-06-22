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
