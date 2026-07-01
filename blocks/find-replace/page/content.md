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

## FAQ

<details>
<summary>Why doesn't \n in the replacement insert a newline?</summary>

In the default literal mode, both the find and replace strings are taken exactly
as typed — `\n` is a backslash followed by an "n". Switch on **Regular
expression** mode if you want escapes like `\n` or `\t` to be interpreted.

</details>

<details>
<summary>How do I reuse part of the match in the replacement?</summary>

Enable regex mode and use capture groups: wrap parts of the pattern in
parentheses, then reference them as `$1`, `$2`, or `${name}` in the replacement.
For example, find `(\w+)@(\w+)` and replace with `$2:$1` turns `user@host` into
`host:user`.

</details>

<details>
<summary>Can I replace only the first occurrence?</summary>

Yes — untick **Replace all**. By default every match is replaced (and the match
count is reported); with it off, only the first match changes and the rest of
the text is left alone.

</details>

<details>
<summary>Is my text sent anywhere?</summary>

No. The search and replacement run entirely in your browser via WebAssembly, so
you can safely paste logs, config files, or anything private.

</details>
