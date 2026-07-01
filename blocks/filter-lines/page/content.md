## About this tool

**Filter Lines** is a friendly `grep` for your browser: paste some text, give a
pattern, and **keep** or **drop** the lines that match.

- **Pattern** — a literal substring by default. Tick **Treat pattern as regex**
  to use a full regular expression (e.g. `^ERROR`, `\d{3}-\d{4}`,
  `(warn|error)`).
- **Mode** — *keep* outputs only the matching lines; *drop* outputs only the
  lines that don't match.
- **Ignore case** — match regardless of upper/lower case.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Pulling just the `ERROR`/`WARN` lines out of a log.
- Removing comment or blank lines (`drop` with `^#` or `^\s*$`).
- Keeping lines that contain a particular id, tag, or keyword.

### FAQ

<details>
<summary>Do I need to escape dots and brackets in my pattern?</summary>

Not by default — the pattern is a plain substring, so `1.2.3` matches only the literal text `1.2.3`. Characters like `.`, `(`, and `[` only become special once you tick **Treat pattern as regex**.

</details>

<details>
<summary>What regex flavor is supported?</summary>

The Rust `regex` syntax: character classes (`\d`, `\w`), anchors (`^`, `$`), alternation (`(warn|error)`), and repetition all work. Backreferences and lookarounds are not supported. An invalid expression returns an "invalid regular expression" error rather than silently matching nothing.

</details>

<details>
<summary>How do I delete blank lines?</summary>

Switch mode to **drop**, tick **Treat pattern as regex**, and use `^\s*$` as the pattern — it matches empty and whitespace-only lines, so only lines with content remain.

</details>

<details>
<summary>Why do I get a "pattern is empty" error?</summary>

The pattern is required — an empty pattern would match every line (or none), which is rarely what you want, so the tool asks for an explicit substring or regex instead of guessing.

</details>
