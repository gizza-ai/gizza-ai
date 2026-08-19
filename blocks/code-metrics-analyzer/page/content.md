## About this tool

Code metrics are a quick way to spot source files that need a closer review before you wire them
into a larger refactor. Paste one source file or snippet and this tool reports the physical line
counts, code/comment/blank split, function-like declarations, approximate cyclomatic complexity,
cognitive complexity, parameter counts, nesting depth, Halstead volume, and a maintainability grade.

Everything runs locally in WebAssembly. The analyzer is intentionally heuristic: it strips comments,
looks for common declaration patterns, and counts branch/loop/boolean decision points without sending
your code to a server or requiring a repository checkout.

## Worked example

Input:

```js
function grade(score) {
  if (score >= 90) return 'A';
  if (score >= 80) return 'B';
  return 'C';
}
```

With **Language = JavaScript** and **Complexity warning threshold = 2**, the summary includes:

```text
Language: javascript
Lines: total 5, code 5, comment 0, blank 0
Functions: 1
Cyclomatic complexity: total 3, average 3.0, max 3
Over threshold (>2): 1

Functions:
- grade (line 1, 5 LOC): CCN 3, cognitive 4, params 1, nesting 1, risk low
```

## What the metrics mean

- **Lines** are physical line counts: total, blank, comment-only, and code lines. A line with both code
  and a trailing comment counts as code, matching common LOC tools.
- **Functions** are declarations detected by language-specific patterns such as `fn name(`, `def name(`,
  `function name(`, `const name = (...) =>`, `func name(`, and common C-family signatures.
- **Cyclomatic complexity (CCN)** starts at 1 and adds branch/loop/case/catch/match points plus `&&`,
  `||`, and ternary `?` decision points.
- **Cognitive complexity** is a lightweight nesting-weighted companion metric. It is useful for ranking
  snippets, but it is not a byte-for-byte implementation of any vendor's AST-based rule set.
- **Maintainability** uses the classic maintainability-index shape, combining Halstead volume,
  complexity, and code lines, then mapping the 0–100 score to an A–F grade.

## Output formats

- **Summary report** gives the file totals and the top function rows in readable text.
- **Function table** shows one markdown-style row per function, with over-threshold functions marked.
- **JSON** includes line counts, complexity totals, maintainability, warnings, and function objects.
- **CSV** emits one function row per line for spreadsheet or CI-script follow-up.

## Limits and edge cases

- This is a pasted-snippet analyzer, not a repository scanner. It does not read a directory tree,
  follow imports, or compute per-file rollups.
- The language detector is a best-effort score over syntax clues. Pick an explicit language when a
  snippet is tiny or intentionally polyglot.
- It strips ordinary `//`, `#`, `--`, and `/* ... */` comments, but does not fully parse strings or
  language-specific docstring grammars.
- Function detection is heuristic. Macro-generated functions, unusual C++ templates, chained closures,
  decorators split far from a signature, and class methods without ordinary parentheses can be missed.
- Complexity numbers are consistent and useful for comparison inside this tool, but full AST analyzers
  can disagree on edge cases.

## FAQ

<details>
<summary>Is this accurate enough to replace lizard, scc, cloc, or a compiler plugin?</summary>

No. It is a local, paste-and-go triage tool for one snippet or file. Use it to spot obvious hotspots
and to compare versions of the same code. For CI gates over a whole repository, use a language-aware
AST tool or repository scanner.

</details>

<details>
<summary>Why does a line with code and a trailing comment count as code?</summary>

That convention avoids double-counting a physical line. A line such as `return value; // done` is still
an executable code line, while a line containing only `// done` is a comment line. The comments are
stripped before branch and function heuristics run.

</details>

<details>
<summary>What threshold should I use for cyclomatic complexity?</summary>

The default is `10`, a common "look closer" threshold. For small pasted snippets, lower values such as
`3` or `5` are useful when you want the function table to flag every branch-heavy example. The number is
configurable because teams vary in how strictly they gate complexity.

</details>

<details>
<summary>Why did it miss a function in my code?</summary>

The analyzer does not build a full AST. It detects common function forms across many languages, but
exotic declarations, macro-generated functions, and deeply chained anonymous callbacks can fall outside
those patterns. The file-level line counts and complexity still reflect the pasted text.

</details>
