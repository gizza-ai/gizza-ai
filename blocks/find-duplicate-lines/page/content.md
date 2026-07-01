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

## FAQ

<details>
<summary>In what order are the duplicated lines listed?</summary>

Most frequent first. When two lines occur the same number of times, the one
that appeared earlier in your text is listed first, so the ordering is stable
and predictable.

</details>

<details>
<summary>If "Ignore case" merges Foo, foo and FOO, which spelling shows in the results?</summary>

The first-seen form. The tool keeps the line exactly as it first appeared
(e.g. `Foo`) as the display text, while counting all case variants toward the
same total.

</details>

<details>
<summary>Why aren't `item` and `  item  ` counted as the same line?</summary>

By default lines are compared byte-for-byte, so surrounding spaces or tabs make
them different. Turn on **Trim whitespace** to strip leading/trailing
whitespace before comparing — then both count together and the trimmed form is
displayed.

</details>

<details>
<summary>What do the total and unique line counts mean?</summary>

**Total lines** is every line in the input (including blanks). **Unique lines**
is the number of distinct lines after your chosen normalization (case folding
and/or trimming). If nothing repeats, the tool simply says no duplicate lines
were found.

</details>
