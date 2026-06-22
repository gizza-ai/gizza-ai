# html-formatter — competitor analysis & differentiation

**Tool:** `gizza-ai/html-formatter` — pretty-print HTML with consistent
indentation and tag formatting.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Prettier / `js-beautify` (html) | Library/CLI | Excellent, but need Node + config; overkill for a quick beautify. |
| Online "HTML beautifier" sites | Web | Common, but most **upload your markup**, are ad-heavy, and vary in correctness. |
| Editor format-on-save | App | Editor-specific; needs the file open in a configured editor. |
| `tidy -i` | CLI | Powerful but a native install and aggressively rewrites markup. |

## How gizza's tool is better / different

1. **Local — markup never uploaded.** Runs in WASM (chat SW + CLI + page). Safe
   for proprietary templates.
2. **Forgiving, HTML-aware (not XML).** Unlike the sibling `xml-formatter` (which
   requires well-formed XML), this handles real HTML: **void elements**
   (`<br>`,`<img>`,…) don't create phantom indent levels, **self-closing tags**,
   **comments**, and the **doctype** are handled.
3. **Quote-safe parsing.** A `>` inside an attribute value (`title="x > y"`)
   doesn't break tag detection.
4. **Preserves whitespace-sensitive blocks.** `<pre>`, `<textarea>`, `<script>`,
   and `<style>` contents are emitted **verbatim**, so code and pre-formatted
   text aren't mangled.
5. **Configurable indent** (0–8 spaces) and three surfaces, one Rust core, no
   dependencies.

## Verification

Eight core unit tests pin the exact output: nesting, void elements staying flat,
self-closing tags, attribute-with-`>`, comment+doctype, `<pre>` verbatim
preservation, and configurable indent width. **End-to-end CLI** beautified
`<div><p>Hello <b>world</b></p><br><img src=x></div>` into a correctly indented
tree (with `<br>`/`<img>` flat). Page Playwright covers nesting + indent width.

## Scope / honest limitations

- Indentation/structure formatter, not a full reflow/wrap engine (it won't
  re-wrap long attribute lists or text). That's Prettier's domain.
- Assumes reasonably balanced tags; it indents by open/close and won't "repair"
  badly broken markup (it won't crash, but output mirrors the input structure).

## Possible future enhancements

- A minify mode (collapse whitespace) to complement pretty mode.
- Optional attribute-per-line wrapping for long tags.
- Inline-element collapsing (keep short `<b>`/`<a>` on the parent line).
