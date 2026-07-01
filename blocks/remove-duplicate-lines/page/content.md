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

### FAQ

<details>
<summary>Where does a line end up when I choose "keep last"?</summary>

At the position of its **last** occurrence. With input `a, b, a`, keeping the first gives `a, b`; keeping the last gives `b, a` — the surviving `a` sits where the final repeat was, and the rest of the order is preserved around it.

</details>

<details>
<summary>How is the consecutive-only mode different from normal deduplication?</summary>

It behaves like Unix `uniq`: only *runs* of identical adjacent lines collapse to one. A line that shows up again later — after different lines in between — is kept. Use it to squash repeated log lines without losing legitimate re-occurrences.

</details>

<details>
<summary>If I combine "ignore case" and "trim", which version of the line is kept?</summary>

The occurrence chosen by your keep setting (first or last), with one tweak: when **trim** is on, the kept line is output with its leading/trailing whitespace removed. Ignore-case only affects *matching* — the kept line's original casing is untouched.

</details>

<details>
<summary>Is there a limit on how much text I can paste?</summary>

No fixed line limit — processing is a single pass in WebAssembly, so even large lists dedupe quickly, and nothing is sent to a server regardless of size.

</details>
