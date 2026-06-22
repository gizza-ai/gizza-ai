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
