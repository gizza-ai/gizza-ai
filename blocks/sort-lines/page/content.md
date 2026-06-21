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
