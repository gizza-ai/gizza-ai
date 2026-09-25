## About this tool

Python Syntax Check parses pasted Python with a Python 3 grammar and reports the first compile-time problem without running the code. Use it when you want a fast `py_compile`-style answer in a browser or from the `gizza` CLI: valid code reports OK, while invalid code gets a `SyntaxError`, `IndentationError` or `TabError` with line, column and the offending line marked by a caret.

The checker is deliberately limited to syntax. It does not execute imports, evaluate expressions, call user functions or lint style rules, so runtime failures such as `NameError` and style guidance such as PEP 8 are out of scope. The optional Python 2 hints explain common migration-only syntax forms such as `print "text"`, `except Error, err:`, backtick repr and legacy octal literals.

Example text output for a missing colon:

```text
broken.py:1:16: SyntaxError: invalid syntax. Got unexpected token Newline

  1 | def greet(name)
    |                ^

Stats
  lines: 2
  non-empty lines: 2
  characters: 31
```

Switch `format` to `json` when you need a machine-readable result for CI or scripts. Use `mode=expression` to check a single `eval()`-style expression, or `mode=interactive` for REPL-style input.

## FAQ

<details>
<summary>Does this run my Python code?</summary>

No. The tool parses the source with a Python 3 grammar and stops there. It never imports modules, opens files, evaluates expressions or calls functions, which keeps it focused on syntax errors rather than runtime behaviour.

</details>

<details>
<summary>Why does it show only one error?</summary>

Python parsers usually stop at the first syntax problem because the rest of the file may be ambiguous after that point. Fix the reported line first, then run the checker again to reveal the next issue if there is one.

</details>

<details>
<summary>What is the difference between module, expression and interactive mode?</summary>

`module` checks a whole `.py` file. `expression` accepts one expression like `1 + len(items)`, similar to `eval()`. `interactive` accepts a REPL-style statement block. If you are checking a script or pasted file, leave the default `module` mode.

</details>

<details>
<summary>Can this check Python 2 syntax?</summary>

It checks Python 3 syntax only. With Python 2 hints enabled, the report may add migration notes for common Python-2-only forms such as `print "hello"`, `<>`, backtick repr and legacy octal literals.

</details>

<details>
<summary>Is this a style linter?</summary>

No. It does not enforce formatting, unused imports, type hints or PEP 8. It answers the narrower question: can this source be parsed as Python 3 code?

</details>
