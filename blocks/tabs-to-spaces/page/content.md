## About this tool

**Tabs to Spaces** converts the whitespace in text or code between tabs and
spaces — the same job as `expand`/`unexpand` — right in your browser. A tab is
never a fixed run of spaces: it advances to the next **tab stop**, the next
column that is a multiple of the tab width, so the result stays aligned.

- **Direction** — *expand* (default) turns tabs into spaces; *unexpand* turns
  runs of spaces back into tabs.
- **Tab width** — columns per tab stop (1–16, default 4). Common values are `2`,
  `4`, and `8` (the Unix default).
- **Scope** — *all* converts both leading indentation and inline whitespace;
  *leading* touches only the indentation before the first non-blank character on
  each line and copies the rest verbatim.

Each line is processed independently and existing line breaks are preserved.
Everything runs **locally via WebAssembly** — your text is never uploaded.

### Worked example

Expanding this input with **tab width 4** and **direction expand**:

```
→if (ok) {
→→return 1;
→}
```

(where `→` marks a tab) produces spaces to the next tab stop on each line:

```
    if (ok) {
        return 1;
    }
```

The first tab (column 0) fills 4 spaces; the doubly-indented line fills 8. A tab
placed after `ab` (column 2) would fill only **2** spaces, because it only needs
to reach column 4 — that is what "respecting tab stops" means.

### Handy for

- Normalizing a snippet to your project's indentation style before pasting.
- Meeting a linter that requires spaces (or tabs) for indentation.
- Cleaning up mixed tabs and spaces in copied code.

## FAQ

<details>
<summary>Why doesn't every tab become the same number of spaces?</summary>

Because a tab advances to the next **tab stop**, not a fixed count. At tab width
`4`, a tab at column 0 becomes 4 spaces, but a tab after `ab` (column 2) becomes
only 2 spaces — both land on column 4. This keeps columns aligned exactly the
way an editor with the same tab width would show them.

</details>

<details>
<summary>What does "leading indentation only" change?</summary>

With **scope** set to *leading*, only the whitespace before the first non-blank
character on each line is converted; any tabs or spaces later in the line are
left exactly as they are. Use it to re-indent code without disturbing aligned
comments or tab-separated data further along the line.

</details>

<details>
<summary>Does unexpand turn single spaces into tabs?</summary>

No. When converting **spaces → tabs**, a tab is only used when it lands on a tab
stop *and* replaces two or more columns. A single leftover space (or a lone
space before a stop) stays a space, so `unexpand` never mangles ordinary word
spacing — only genuine indentation runs collapse into tabs.

</details>

<details>
<summary>Are my line breaks and non-whitespace characters preserved?</summary>

Yes. Every line is processed independently, and line breaks are kept exactly as
in the input. Only tabs and spaces are rewritten; all other characters pass
through unchanged.

</details>

## Limits & edge cases

- **Tab width** is clamped to `1–16`. A value of `0` becomes `1`, and anything
  above `16` becomes `16`.
- Only the tab character (`\t`) and the space character are treated as
  whitespace; other Unicode spaces are left as-is and each counts as one column.
- Very large pastes run entirely in your browser, so speed depends on your
  device — but nothing is ever sent to a server.
