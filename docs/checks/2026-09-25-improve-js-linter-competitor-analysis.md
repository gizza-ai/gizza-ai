# js-linter — competitor analysis (2026-09-25)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is a
**paraphrase** of publicly visible tool behaviour — no competitor copy, branding, trademarks,
rule text or assets were copied. Out-of-model items are *listed*, never built.

Search used: "online javascript linter tool paste code check errors eslint online".

## Competitors reviewed

Four candidates were opened; one (codeshack.io/js-validator) returned HTTP 403 to a plain fetch
and one (webdesignsutra.com) serves an empty client-rendered shell, so both were **replaced** with
the next real tools in the result list rather than running the scan short.

### 1. ValidateJavaScript — validatejavascript.com

- **Features:** ESLint-backed checking of JS and JSX, with an automatic-fix pass.
- **Params / options:** configuration preset (custom · a popular strict community style guide ·
  the ESLint recommended defaults · a large-vendor style guide · a semicolon-free community
  style), an "auto-fix" toggle, and an environment selector.
- **Input:** pasted code in an editor pane. **Output:** an itemised error list.
- **UX patterns:** single primary action button, preset dropdown, boolean toggle, keyboard
  shortcut to re-run.
- **Limits stated:** none published.
- **Free vs paid:** free, no account.

### 2. Devsly — devsly.io/javascript/linter

- **Features:** live analysis, several rule sets, per-line reports with suggested fixes, ES6+
  awareness, complexity reporting.
- **Params / options:** ECMAScript version (ES5 → latest), environment (browser · Node · both),
  source type (script · module), plus individual rule toggles for semicolons, quote style,
  indentation, unused variables, undefined variables, camelCase, strict equality, curly braces,
  `console`/`debugger`/`alert` usage, and `const`/`let` over `var`. Quick presets: recommended,
  two community style guides, minimal, strict.
- **Output:** error/warning/info counts plus line-by-line entries carrying severity, rule name
  and an explanation.
- **UX patterns:** paste-and-analyse flow, split input/output panes, live re-config, one primary
  action button, preset chips.
- **Limits stated:** JavaScript only; TypeScript needs a different tool.

### 3. JsonToTable — jsontotable.org/javascript-linter

- **Features:** style issues (spacing, naming), likely bugs (unused variables, missing
  semicolons), and best-practice/modernisation hints.
- **Output:** grouped by severity — errors, warnings, suggestions — with line numbers and a short
  message per issue.
- **UX patterns:** a "load a sample with problems in it" button, `.js` file upload, paste input,
  live feedback, copy/download of the result.
- **Limits stated:** free, no size cap, no sign-up.

## Table stakes → where each one landed

| Table stake (≥1 competitor ships it) | Verdict | Where it landed |
| --- | --- | --- |
| ECMAScript target version | **in-model** | `ecma` = `es5 \| es2015 \| es2020 \| latest` (default `latest`); drives `ES-VERSION` and `NO-VAR` |
| Environment / known globals | **in-model** | `env` = `browser \| node \| both \| none` (default `browser`); drives `UNDEF-VAR` |
| Script vs module source type | **in-model** | `source_type` = `auto \| script \| module` (default `auto`); drives `MODULE-SYNTAX` |
| Rule-set presets (recommended/minimal/strict) | **in-model** | `preset` = `recommended \| minimal \| strict` (default `recommended`) — selects which rule families run |
| Severity levels + counts in the report | **in-model** | three severities; header carries total/error/warning/info counts |
| Severity filtering | **in-model** | `min_severity` = `all \| warning \| error` |
| Per-rule suppression (their per-rule toggles) | **in-model** | `ignore` — comma/space-separated rule codes. One list field beats 12 booleans of schema bloat, and matches the sibling `shell-script-linter` family invariant |
| Line numbers + rule id + message + source line | **in-model** | every finding carries all four |
| Machine-readable output for CI | **in-model** | `format` = `text \| json` |
| Unused variables | **in-model** | `UNUSED-VAR` |
| `==` vs `===` | **in-model** | `EQEQ` |
| Unreachable code | **in-model** | `UNREACHABLE` |
| Undeclared / implicit globals | **in-model** | `UNDEF-VAR` |
| `var` over `const`/`let` | **in-model** | `NO-VAR` |
| Missing semicolons | **in-model** | `SEMICOLON` |
| `console` / `debugger` / `alert` left in | **in-model** | `NO-CONSOLE`, `NO-DEBUGGER`, `NO-ALERT` |
| Curly braces on single-statement bodies | **in-model** | `CURLY` |
| Syntax errors with a location | **in-model** | `SYNTAX` (unbalanced brackets, unterminated string/template/comment) |
| Sample / preset buttons | **in-model** | four `[[example]]` chips on the page |
| Copy the result | **in-model** | platform-provided Copy button |
| Live re-run while editing | **in-model** | the generated page re-runs on every field change |

### Out-of-model — considered, deliberately not built

- **Automatic fixing / "download the fixed file".** Safe rewriting needs a real AST with exact
  source ranges. A token-level checker cannot edit code without risking corruption, so this tool
  reports and leaves the edit to the user. Stated as a limit on the page.
- **JSX / React syntax.** Needs a full JSX-aware parser; the tokenizer here would misread the
  angle-bracket forms. Stated as a limit.
- **TypeScript.** Same reason, and every competitor scanned also declines it.
- **`.js` file upload.** Pure-compute pages in this toolkit are field-only; the file-input control
  belongs to the ffmpeg page runtime. Paste covers the same job.
- **Indentation / quote-style / camelCase formatting rules.** These are a *formatter's* job and
  the toolkit already ships `js-beautify`; duplicating them here would be a worse version of an
  existing tool.
- **Cyclomatic-complexity scoring.** Needs real control-flow analysis over an AST, not a token
  stream — out of reach for a heuristic checker, and a bad-quality answer is worse than none.
- **Accounts, saved configs, shareable config URLs.** Requires a backend; the toolkit is
  browser-local with no account. (Parameter deep-links via `?code=…&preset=…` cover the
  share-a-run case.)

## Feasibility spike (done before tagging anything out-of-model)

Spiked a hand-written JavaScript tokenizer in Rust — line/block comments, single/double-quoted
strings with escapes, template literals with nesting, numeric literals, identifiers, punctuators,
and the classic regex-literal-vs-division disambiguation (a `/` opens a regex unless the previous
significant token can end an expression). That is ~200 lines with no new dependencies and makes
every rule above robust against code that merely *mentions* a pattern inside a string or comment.
This is why `UNUSED-VAR`, `UNDEF-VAR`, `DUP-KEY` and `UNREACHABLE` are tagged in-model rather than
dismissed as "needs a parser" — the spike showed a token stream plus brace-depth scope tracking is
enough. Full AST-only capabilities (auto-fix, complexity) stayed out.
