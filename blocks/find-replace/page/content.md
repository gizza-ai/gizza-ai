## Find and replace text in your browser

Paste your text, enter what to **find** and what to **replace** it with, and get
the result instantly. Everything runs locally in your browser — your text is
never uploaded to a server.

### Options

- **Regular expression** — off by default (the find term is matched literally,
  so `.` `*` `(` etc. have no special meaning). Turn it on to use a regex, and
  reference capture groups in the replacement with `$1`, `$2`, or `${name}`.
- **Case sensitive** — on by default. Turn it off to match regardless of case.
- **Replace all** — on by default (every match is replaced). Turn it off to
  replace only the first match.

### Examples

- Literal: find `, ` replace `\n` is *not* a newline (literal mode) — use regex
  mode for escapes.
- Regex: find `(\w+)@(\w+)` replace `$2:$1` rewrites `user@host` → `host:user`.
- Regex: find `\s+` replace ` ` collapses runs of whitespace into single spaces.

Leave **Replace with** blank to simply delete every match.
