## What this tool does

Paste a bash, sh, dash or zsh script and get a static lint report without running the script. The checker focuses on high-signal shell pitfalls that are easy to miss in review: unquoted variable expansions, missing strict mode, useless `cat`, legacy backticks, pipe-to-while subshell scope traps, unchecked `cd`, parsing `ls`, spaced assignments, risky `rm -rf` paths, simple block/quote syntax mismatches and POSIX-shell bashisms.

Each finding includes a line number, severity, rule code, message and the source line that triggered it. Use **Text report** while editing, or **JSON for CI** when you want summary counts and a machine-readable findings array.

## Worked example

Input:

```bash
#!/usr/bin/env bash
name=$1
echo Hello $name
for f in $(ls *.txt); do
  cat $f | while read line; do
    echo $line
  done
done
```

Typical output:

```text
Shell: bash
Findings: 6 (0 errors, 5 warnings, 1 info)

L2 [warning] STRICT-MODE: missing strict mode: set -e, set -u, set -o pipefail
  name=$1
L3 [warning] UNQUOTED-VAR: quote variable expansions to avoid word splitting and globbing
  echo Hello $name
L4 [warning] PARSE-LS: do not parse ls output; glob directly or use find -print0/read -d ''
  for f in $(ls *.txt); do
L5 [info] USELESS-CAT: useless use of cat; redirect the file into the consumer instead
  cat $f | while read line; do
L5 [warning] SUBSHELL-SCOPE: commands after a pipeline run in a subshell in many shells; variables assigned in the loop are lost
  cat $f | while read line; do
L6 [warning] UNQUOTED-VAR: quote variable expansions to avoid word splitting and globbing
  echo $line
```

The script is never executed; only its text is scanned. Comments, single-quoted strings and here-doc bodies are masked before linting so examples in documentation do not produce findings.

## Rule codes

| Code | What it catches |
| --- | --- |
| `SYNTAX` | Unclosed `if`/`for`/`while`/`case` blocks, unmatched `fi`/`done`/`esac`, and unterminated quotes. |
| `MISSING-SHEBANG` | A script that does not start with `#!`. |
| `STRICT-MODE` | Missing `set -e`, `set -u` or `set -o pipefail`. |
| `UNQUOTED-VAR` | Unquoted `$name` / `${name}` expansions in command text. |
| `USELESS-CAT` | `cat file | command` pipelines. |
| `BACKTICKS` | Legacy command substitution using backticks. |
| `SUBSHELL-SCOPE` | `producer | while read ...` loops whose body runs in a subshell in many shells. |
| `UNCHECKED-CD` | `cd path` without `&&`, `||`, `set -e`, or an explicit guard. |
| `PARSE-LS` | `for f in $(ls ...)`, which breaks on whitespace and unusual filenames. |
| `ASSIGN-SPACES` | `VAR = value`, which runs a command named `VAR` instead of assigning. |
| `LEGACY-TEST` | Single-bracket `[ ... ]` tests in bash/zsh scripts, where `[[ ... ]]` avoids splitting surprises. |
| `RM-RISK` | `rm -rf` against an interpolated path. |
| `SH-BASHISM` | Bash-only arrays, `[[ ... ]]`, process substitution or here-strings under `sh`/`dash`. |

## Limits and edge cases

- This is a lightweight, deterministic pitfall checker, not a full shell parser. Keep a full linter in CI for production-critical scripts.
- The `shell = auto` mode reads the shebang and falls back to bash when there is none. Choose `sh` or `dash` to enable POSIX bashism checks.
- `ignore` accepts comma- or space-separated rule codes, such as `LEGACY-TEST, USELESS-CAT`.
- Input is capped at 200,000 bytes so the browser page stays responsive.
- Here-doc bodies, comments and single-quoted strings are masked. Double-quoted strings are still scanned because expansions inside them are meaningful.
- The checker reports one finding per rule per line to keep the output readable.
- It does not apply fixes. Shell rewriting needs an exact AST and source ranges; this tool reports the issue and leaves the edit to you.

## FAQ

<details>
<summary>Is this a replacement for ShellCheck?</summary>

No. It catches a practical subset of common mistakes and runs entirely inside this toolkit, but it is intentionally heuristic. Use it for quick local review and CI-friendly JSON output, and keep a full grammar-aware shell linter in your release pipeline.

</details>

<details>
<summary>Does the tool execute my script?</summary>

No. The script is treated as plain text and scanned in Rust/WebAssembly. Commands, command substitutions and variables are not evaluated, no files are read, and no network calls are made.

</details>

<details>
<summary>How do I hide a finding I have reviewed?</summary>

Put the rule code in **Ignore rule codes**, separated by commas or spaces. For example, `LEGACY-TEST USELESS-CAT` hides those two rule families while still reporting syntax errors, strict-mode gaps and other warnings.

</details>

<details>
<summary>Why does POSIX sh report different findings from bash?</summary>

Bash and zsh accept features such as arrays, `[[ ... ]]`, process substitution and here-strings. `sh` and `dash` do not, so choosing a POSIX dialect enables `SH-BASHISM` and disables the bash-specific `LEGACY-TEST` hint.

</details>

<details>
<summary>Can I use it in CI?</summary>

Yes. Choose **JSON for CI** or pass `format=json` on the CLI. The output includes total, error, warning and info counts plus a findings array with line, severity, code, message and source fields.

</details>
