## About this tool

**JavaScript Linter** checks pasted `.js` source for high-signal problems before you commit or share a snippet. It runs locally in WebAssembly and reports line numbers, severities, rule IDs, messages and the source line that triggered each finding.

### What it checks

- **SYNTAX** — unmatched brackets/braces and unterminated string, template or block-comment patterns.
- **EQEQ** — `==` and `!=` where strict equality is safer.
- **UNREACHABLE** — statements after `return`, `throw`, `break` or `continue` in the same block.
- **UNUSED-VAR** and **UNDEF-VAR** — simple declaration/assignment mistakes.
- **NO-VAR** — `var` in modern ECMAScript targets where `let` or `const` is clearer.
- **SEMICOLON**, **CURLY**, **NO-CONSOLE**, **NO-DEBUGGER** and **NO-ALERT** for common style and debugging leftovers.
- **MODULE-SYNTAX** when `import` or `export` appears while `source_type` is `script`.

Use **preset** to switch between a small bug-focused set (`minimal`), the balanced default (`recommended`) and the full style-oriented set (`strict`). Use **ignore** for a comma- or space-separated list of rule IDs you do not want in a particular run, such as `NO-CONSOLE SEMICOLON`.

### Worked example

Input:

```js
function demo(x) {
  var unused = 1
  if (x == 1) console.log(x)
  return x;
  alert('later');
}
```

With `preset=recommended`, `ecma=latest`, `env=browser`, `source_type=auto`, `min_severity=all` and `format=text`, the report includes `NO-VAR`, `UNUSED-VAR`, `EQEQ`, `CURLY`, `SEMICOLON`, `NO-CONSOLE` and `UNREACHABLE` findings.

### Limits and edge cases

This is a dependency-free heuristic checker, not a full ESLint replacement. It intentionally does not auto-fix code, parse JSX or TypeScript, calculate complexity metrics, or enforce formatting rules already covered by the JavaScript beautifier tool. Strings and comments are masked before checks, so examples inside comments do not usually trigger rules, but very unusual JavaScript grammar can still require a real AST-aware linter.

Everything runs **locally in your browser**; your code is not uploaded.

## FAQ

<details>
<summary>Is this a replacement for ESLint?</summary>

No. It is a fast, browser-local checker for common mistakes and copy-pasted snippets. Use ESLint in a project when you need full AST parsing, plugins, shareable config files, automatic fixes, JSX or TypeScript support.

</details>

<details>
<summary>Why are JSX and TypeScript out of scope?</summary>

Both add syntax that changes how tokens such as angle brackets, types and imports should be parsed. This tool stays dependency-free and conservative; it reports JavaScript issues without pretending to understand languages that need a different parser.

</details>

<details>
<summary>How do I suppress a rule for one run?</summary>

Put the rule IDs in the **Ignore rule IDs** field, separated by spaces or commas. For example, `NO-CONSOLE SEMICOLON` keeps console calls and missing semicolons out of the report while still showing the other rules.

</details>

<details>
<summary>Can I use the JSON output in scripts?</summary>

Yes. Set `format=json` to receive an object with an `issues` array. Each issue includes `line`, `column`, `severity`, `rule`, `message` and `source`, which is convenient for quick local checks or CI glue.

</details>
