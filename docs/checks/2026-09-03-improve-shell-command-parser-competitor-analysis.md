# shell-command-parser competitor analysis (2026-09-03)

## Tool under review

- Slug: `shell-command-parser`
- Goal: parse a shell command line into a structured view of commands, pipes, redirects, assignments, quoting, globs, and expansions without executing it.
- Model fit: pure Rust parser/renderer, no network, no command execution.

## Sources skimmed

| Source | Observed table-stakes | In model? | Decision |
| --- | --- | --- | --- |
| JLV Extension Terminal Command Explainer | Paste a command, break it into base command/flags/arguments/paths/pipes/redirects, plain-English explanation | Yes | Ship text input plus `explain`, `tree`, and JSON renderers. |
| ShellRAG Bash pipes/redirection tutorial | Explains pipes, stdout/stderr/stdin redirection, append, heredocs, process substitution | Yes | Parser detects pipes, fd redirections, here-docs, here-strings, and process substitutions. |
| Dev Toolbox bash pipes/redirects examples | Worked examples for `|`, `>`, `>>`, `2>`, `<`, heredocs, process substitution | Yes | Include redirects in AST and command table; page examples cover pipe/redirect cases. |
| utils.com Shell Command Explainer | Paste-and-explain UX, argument breakdown, clear descriptions for shell components | Yes | Descriptor offers selectable formats and concise page copy with examples. |

## Parameter and UX decisions

| Capability | Default / control | In model? | Implemented as |
| --- | --- | --- | --- |
| Paste arbitrary command, including newlines | Multiline textarea | Yes | Required `input` string param with placeholder example. |
| Choose output style | Select / enum | Yes | `format = json|tree|explain|commands`, default `json`. |
| Human-readable explanation | Preset/format choice | Yes | `explain` renderer. |
| Structured machine output | Pretty JSON by default | Yes | JSON renderer plus `pretty` checkbox. |
| Compact command inventory | Table output | Yes | `commands` format lists command, args, redirects, env. |
| Execute or validate effects of a command | N/A | Out of model | Explicitly not built; this tool parses only and does not run commands. |
| Full bash control-flow grammar | N/A | Out of model for this iteration | Reserved words are identified with notes; full `if/for/case` AST nesting is not attempted. |

## Verification implications

- Unit tests should cover happy paths for pipes, redirects, quoting, expansions, heredocs, process substitution, and errors for unterminated quotes/dangling operators.
- CLI and page checks should exercise at least one exact text output (`commands` or `tree`) and one deep-linked page run.
- The page must present the format enum as a select and `pretty` as a checkbox so users can mirror competitor output styles without editing JSON manually.
