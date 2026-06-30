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
