# html-validate — competitor analysis (2026-07-30)

Snapshot before finishing `blocks/html-validate`: validate an HTML document/snippet and report syntax errors, unclosed tags, and nesting issues. Competitor copy is paraphrased only.

## Competitors profiled

Search query: `online HTML validator unclosed tags nesting errors line column HTML checker`.

| # | Tool | Table-stakes observed | Decision |
|---|------|-----------------------|----------|
| 1 | CodeShack HTML Validator | Online paste box, live validation, unclosed tags, duplicate IDs/attributes, empty `src`, missing `alt`, exact line information, local/browser positioning. | Build the syntax/tag/nesting subset; duplicate-id/a11y checks are out of scope for this block. |
| 2 | Hostt HTML Tag Checker | Highlights unclosed tags, mismatched tags, improper nesting, precise line numbers. | Build directly: unclosed, unexpected close, overlapping/misnested, line:column. |
| 3 | HTML Nest Validator | Focuses specifically on nesting, unclosed tags, and syntax errors. | Matches this backlog row; keep the gizza surface simple and deterministic. |
| 4 | HtmlFormatter HTML Validator | Checks syntax, tag matching, attribute validity, structure, unclosed tags, invalid nesting, malformed markup. | Build syntax/tag/nesting checks; full HTML5 attribute validity is out-of-model without a large spec table. |
| 5 | HTML Editor Online validator | Checks common issues including missing required elements, unclosed tags, deprecated markup, accessibility missing-alt, and invalid nesting. | Defer full spec/a11y/deprecated-rule checks; this block is a structural tag validator. |

## Table stakes

- Paste a full document or snippet into a multiline field. **In-model.**
- Human-readable report that says valid/invalid and counts errors/warnings. **In-model.**
- Line and column for every issue. **In-model.**
- Detect unclosed tags. **In-model.**
- Detect mismatched/misnested tags (`<b><i></b></i>`). **In-model.**
- Detect stray close tags (`</span>` with no opener). **In-model.**
- Detect basic syntax errors such as unterminated tags/comments and nameless tags. **In-model.**
- Machine-readable JSON output for automation. **In-model.**
- Understand void/self-closing elements, quoted attributes, and raw script/style/pre/textarea content. **In-model.**

## UX controls and defaults

- Main control: large multiline HTML textarea with an example placeholder. **Built.**
- Output format select: `report` default plus `json`. **Built.**
- Preset chips: misnested tags, unclosed tags, valid snippet, JSON output. **Built.**
- Default report format is text so a human immediately sees line/column issues. **Built.**

## Out-of-model / deliberately not built

- Full WHATWG/HTML5 conformance validation: requires a large evolving spec rule database.
- Accessibility checks such as `img[alt]`: useful, but a separate accessibility/lint tool rather than pure syntax/nesting validation.
- Duplicate IDs/attributes and deprecated-element policy checks: possible future enhancements, but beyond the picked row's syntax/unclosed/nesting scope.
- Network fetching a URL: this repo's local page/CLI pattern is paste-input only for pure text validators.

## Existing-tool differentiation

- `html-formatter` and `code-formatter` re-indent or beautify markup; they do not return a validation report with line:column errors.
- `html-minifier` compresses HTML and is not a syntax/nesting validator.
- `format-validator` is broad file-format detection/validation, not HTML-specific stack/nesting diagnostics.
