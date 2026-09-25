## About this tool

Shell Command Parser turns a shell command line into a readable parse without executing it. Use it to audit copy-pasted commands, document build scripts, explain pipelines to teammates, or see exactly which words are arguments, redirects, environment assignments, glob patterns, and shell expansions.

Worked example:

```sh
LC_ALL=C grep -rn 'FIXME' src/ 2>/dev/null | sort -u > matches.txt
```

The parser reports two commands joined by a pipe, `LC_ALL=C` as an environment assignment for `grep`, `2>/dev/null` as a stderr redirection, `'FIXME'` as a single-quoted argument whose value is `FIXME`, and `> matches.txt` as the final stdout file redirection.

Limits and edge cases: the input limit is 200,000 bytes and nesting is capped at 32 levels. Commands are parsed, not executed; parameter expansion, command substitution, arithmetic expansion, process substitution, and globs are detected but never evaluated. Reserved words such as `for`, `if`, and `case` are marked as control-flow keywords rather than expanded into a full shell grammar.

## FAQ

<details>
<summary>Does this tool execute my command?</summary>

No. The command is treated as text. Expansions such as `$HOME`, `$(date)`, `<(sort file)`, and `*.txt` are reported in the parse tree, but they are not resolved and no process is started.

</details>

<details>
<summary>Which shell syntax is supported?</summary>

It focuses on POSIX and common bash command-line syntax: simple commands, environment assignments, arguments, pipes, `&&`/`||`, background `&`, subshells, brace groups, redirects, here-documents, here-strings, quotes, globs, and common expansion forms. It intentionally does not fully model shell control-flow bodies.

</details>

<details>
<summary>Why are `$HOME` or `*.txt` still shown literally?</summary>

A shell parser should distinguish syntax from runtime expansion. This tool removes quote characters for the `value` field, but leaves parameter expansions, command substitutions, arithmetic expressions, process substitutions, and globs as source text so you can see what the shell would expand later.

</details>

<details>
<summary>Which output format should I choose?</summary>

Use JSON when another tool needs structured data, Tree when you want a compact visual shape, Explain for a prose walkthrough, and Commands for a flat table of executable names, arguments, redirects, and environment assignments.

</details>
