## About this tool

**Remove Duplicate Lines** takes pasted text and returns it with duplicate lines
removed. By default the **first** occurrence of each line is kept and the original
order is preserved — so the output is paste-ready.

- **Keep occurrence** — keep the **first** (default) or the **last** occurrence of
  each repeated line.
- **Ignore case** — treat `Foo`, `foo`, and `FOO` as the same line.
- **Trim whitespace** — ignore leading/trailing spaces and tabs when comparing, so
  `  item` and `item` collapse together (the kept line is also trimmed).
- **Only consecutive duplicates (uniq)** — only collapse *runs* of identical lines,
  like the Unix `uniq` command; a line that reappears later is kept.
- **Remove blank lines** — also drop empty (or whitespace-only) lines.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Cleaning up lists, exports, logs, and word/URL/email collections.
- Deduplicating a file before importing it somewhere.
- Collapsing repeated log lines with the consecutive-only (`uniq`) mode.

> Want to *count* the repeats instead of removing them? Use the **Find Duplicate
> Lines** tool, which lists each repeated line with its count.
