## About this tool

**Find Duplicate Lines** scans pasted text and lists the lines that appear **more
than once**, along with how many times each occurs — sorted most-frequent first.

- **Ignore case** — treat `Foo`, `foo`, and `FOO` as the same line.
- **Trim whitespace** — ignore leading/trailing spaces and tabs when comparing,
  so `  item` and `item` count together.

It also reports the total number of lines and the number of unique lines.
Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Spotting repeated entries in a list, export, or log.
- Quality-checking a deduplicated file (there should be none left).
- Finding the most common repeated value in a column of data.

> Want to *remove* the duplicates instead of just counting them? Use a dedupe
> tool; this one is for **finding and counting** repeats.
