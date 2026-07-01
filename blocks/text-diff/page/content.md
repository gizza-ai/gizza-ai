# Text Diff

Compare two blocks of text line by line and return either a classic unified diff or a structured JSON report. It is useful for reviewing snippets, config changes, prompts, small documents, and pasted logs without leaving the browser.

## Inputs

- **Original text** — the left/old version.
- **Updated text** — the right/new version.
- **Output format** — `unified` for a familiar patch-style diff, or `json` for counts and machine-readable line operations.
- **Ignore case** — match lines case-insensitively while preserving original text in the output.
- **Ignore whitespace** — normalize whitespace for matching while preserving original text.
- **Context lines** — unchanged lines to include around each hunk in unified output.

Everything runs locally in WebAssembly.

## FAQ

<details>
<summary>What does the "context lines" setting do?</summary>

It controls how many unchanged lines are shown around each change hunk in the
unified output — 3 by default, like `git diff`, and anywhere from 0 to 100. Set
it to 0 to see only the changed lines, or raise it when you need more
surrounding text to locate a change in a big file.

</details>

<details>
<summary>If I ignore case or whitespace, what appears in the diff?</summary>

The original text. Those options only affect *matching*: `Hello` and `hello`
compare equal with ignore-case on, so the line isn't reported as changed, but
whatever is shown always keeps its exact original casing and spacing.

</details>

<details>
<summary>When should I pick JSON output instead of unified?</summary>

Unified is the familiar patch-style view for human review (and can be applied
with `patch`-style tooling). JSON gives you added/removed/changed/unchanged
counts and a machine-readable list of line operations — the right choice when a
script or another tool consumes the result.

</details>

<details>
<summary>Is the diff computed word-by-word or line-by-line?</summary>

Line-by-line. Each line is treated as a unit, so a one-character edit marks the
whole line as removed-and-added. For prose where you want word-level changes,
split the text so each sentence or clause is on its own line first.

</details>
