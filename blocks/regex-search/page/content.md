## About this tool

Regex Search is a browser-based `grep` for pasted text. Drop in a log, a config
file, or any block of text, type a regular expression (or a plain literal
string), and get back **every whole line that matches** — with 1-based line
numbers, an optional window of surrounding context lines, and match
highlighting. It runs entirely in your browser with pure-Rust WebAssembly:
nothing you paste is uploaded anywhere.

It mirrors the flags you already know from Unix `grep`: ignore case (`-i`),
whole-word only (`-w`), invert to show the lines that *don't* match (`-v`),
fixed-string/literal mode (`-F`), context lines (`-C`), and line numbers (`-n`).

### Worked example

Paste this text:

```
starting up
ERROR disk full
retrying
ERROR timeout
done
```

with the pattern `ERROR` and line numbers on, and you get:

```
2 matching lines:
2:ERROR disk full
4:ERROR timeout
```

Each hit line is prefixed with `line-number:`. Turn on **Context lines** and the
surrounding lines appear with a `line-number-` prefix, and non-adjacent groups
are separated by a `--` line — exactly like `grep -C`.

### Limits & notes

- Matching uses [Rust `regex`](https://docs.rs/regex) syntax. It is fast and
  linear-time, but it does **not** support backreferences or look-around
  assertions (`\1`, `(?=…)`, `(?<=…)`); rewrite those patterns without them.
- Search is line-oriented: `^` and `$` anchor the start and end of each line, so
  a pattern never matches across a line break.
- Highlighting wraps matches in `«` and `»` markers (the output is plain text,
  so it stays copy-paste safe); context lines are never highlighted.
- To pull out the matched *substrings* or a capture group instead of whole
  lines, use the regex-extract tool; to debug a pattern's spans and capture
  groups, use the regex-tester tool.

## FAQ

<details>
<summary>What is the difference between this and a "find in page" search?</summary>

Browser find highlights matches in place; this returns a clean, copy-pasteable
list of just the matching lines, with line numbers and an optional context
window. It also understands full regular expressions, invert, whole-word, and
case-insensitive matching — the way `grep` does on the command line.

</details>

<details>
<summary>Which regex syntax is supported?</summary>

The Rust `regex` crate syntax: character classes (`[a-z]`, `\d`, `\w`, `\s`),
anchors (`^`, `$`, `\b`), quantifiers (`*`, `+`, `?`, `{2,5}`), groups, and
alternation (`ERROR|WARN`). Backreferences and look-around (`\1`, `(?=…)`) are
**not** supported, which is what keeps matching guaranteed linear-time even on
large inputs.

</details>

<details>
<summary>How do I search for a literal string with special characters?</summary>

Turn on **Literal**. In literal mode every regex metacharacter is escaped, so a
pattern like `a.c` matches only the exact text `a.c` and not `abc`. It's the
equivalent of `grep -F`.

</details>

<details>
<summary>Can I show a few lines around each match?</summary>

Yes — set **Context lines** to the number of lines you want before and after
each hit (like `grep -C N`). Context lines are prefixed with `-` instead of `:`,
overlapping windows are merged, and separate groups are divided by a `--` line.

</details>

<details>
<summary>Is my text uploaded to a server?</summary>

No. All matching happens locally in your browser via WebAssembly. The text you
paste never leaves your machine.

</details>
