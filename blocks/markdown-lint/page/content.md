## About this tool

**Markdown Linter** checks your Markdown for common style and consistency
problems and can **auto-fix** them. Paste a document, pick a mode, and either get
a clear list of issues or a cleaned-up version back.

### Mode

- **check** — report every issue found, one per line, as `line:col  rule  message`
  (with a summary count and how many are auto-fixable).
- **fix** — return the corrected Markdown with all fixable issues applied. Any
  issue that needs a human decision is listed in a trailing HTML comment.

### What it checks

- **MD001 — Heading levels increment by one** (catches a skipped level like H1 → H3).
- **MD009 — Trailing whitespace** (a deliberate two-space hard line break is kept).
- **MD010 — Hard tabs** (expanded to spaces on fix).
- **MD012 — Multiple consecutive blank lines** (collapsed to one).
- **MD018 — No space after `#` on a heading** (`#Heading` → `# Heading`).
- **MD019 — Multiple spaces after `#`** (`#   Heading` → `# Heading`).
- **MD022 — Heading not preceded by a blank line** (a blank line is inserted).
- **MD025 — Multiple top-level H1 headings** (flagged for review).
- **MD026 — Trailing punctuation in a heading** (the `.`/`,`/`;`/`:`/`!` is removed).
- **MD004 — Inconsistent unordered list markers** (`*`, `+`, `-` normalized to the
  first one used).
- **MD040 — Fenced code block missing a language** (flagged for review).
- **MD047 — Missing final newline** (added; trailing blank lines removed).

Fenced code blocks (` ``` ` and `~~~`) are respected, so prose rules like heading
spacing and trailing punctuation never fire on code — but whitespace rules (tabs,
trailing spaces) still apply inside them.

Everything runs **locally in your browser** via WebAssembly — your Markdown is
never uploaded.

### Handy for

- Cleaning up README and docs before committing.
- Enforcing a consistent house style across a docs folder.
- Quickly spotting why a Markdown renderer is misbehaving (stray tabs, bad heading
  spacing, mixed list markers).

## FAQ

<details>
<summary>Why does fix mode leave some issues unfixed?</summary>

Three rules need a human decision: a skipped heading level (MD001), multiple
H1s (MD025) and a code fence with no language (MD040) can each be "fixed" in
more than one valid way. Fix mode applies everything mechanical and lists
these remaining items in an HTML comment at the end of the output, so the
document still renders cleanly.

</details>

<details>
<summary>Will the linter strip my two-space hard line breaks?</summary>

No. MD009 flags trailing whitespace, but a line ending in **exactly two
spaces** is the standard Markdown hard line break and is deliberately kept —
only other trailing spaces (one, or three-plus) are reported and removed.

</details>

<details>
<summary>Does it lint inside code blocks?</summary>

Partially, on purpose. Prose rules (heading spacing, trailing punctuation,
list markers) never fire inside ``` / ~~~ fences — a `# comment` in a shell
snippet is not a heading. Whitespace rules (hard tabs MD010, trailing spaces
MD009) still apply there, since they bite in code too.

</details>

<details>
<summary>Is this the same rule set as markdownlint?</summary>

It implements a 12-rule subset of the widely used markdownlint (MD0xx) rules —
the high-signal ones listed above — with the same rule IDs, so findings map
directly onto markdownlint documentation. There's no config file; the rules
run with their common defaults.

</details>
