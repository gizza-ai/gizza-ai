# Improve truncate-text — competitor analysis (2026-06-30)

## Scope

Tool: `truncate-text`

Goal: shorten text to a chosen number of Unicode characters or words, append a configurable ellipsis only when text is actually cut, and optionally avoid splitting words.

## Competitor scan

1. lodash `truncate`
   - Strengths: widely used JavaScript utility with separator/omission options.
   - Gaps closed here: no coding required, browser/CLI/chat surfaces, word-count mode, and explicit ellipsis-budget control.

2. npm `truncate` / `truncate-html` packages
   - Strengths: reusable app dependencies and some HTML-aware variants.
   - Gaps closed here: dependency-free local tool for plain text snippets, with deterministic Unicode-character counting and tests across gizza surfaces.

3. Online text truncators / snippet generators
   - Strengths: paste-and-copy workflows for meta descriptions and previews.
   - Gaps closed here: private local execution, word-safe cutting, custom suffixes, character or word limits, and CLI/chat automation.

4. CMS/editor excerpt fields
   - Strengths: integrated with publishing workflows.
   - Gaps closed here: standalone conversion before pasting into any CMS, plus exact control over whether the ellipsis counts toward the length.

5. Spreadsheet/formula approaches
   - Strengths: bulk operations in existing data tables.
   - Gaps closed here: no formula setup and readable word-boundary behavior for prose.

## In-model improvements included

- Character and word truncation modes.
- Word-safe character truncation by backing up to whitespace.
- Optional hard character cuts via `break_words=true`.
- Custom ellipsis/suffix.
- Option to count the ellipsis toward the character budget.
- Does not append an ellipsis when input already fits.
- Unicode-character aware (uses Rust `char` iteration rather than bytes).
- Page copy and controls for previews, snippets, and meta descriptions.

## Out-of-model / not built

- HTML/Markdown-aware tag-preserving truncation. This tool treats input as plain text; an HTML excerpt tool should be separate.
- Display-width/grapheme-cluster limits for CJK/emoji/combining marks. This tool counts Unicode scalar values, matching the stated character mode.

## Verification checklist

- Core unit tests cover already-short text, character truncation, word-safe backing up, hard cuts, word truncation, custom ellipsis, Unicode, aliases, bad lengths, and empty input.
- Drift-guard schema test covers the chat/LLM descriptor.
- Web wrapper exposes `run(text, length, unit, ellipsis, count_ellipsis, break_words)`.
- Playwright tests cover default character truncation, word mode/custom suffix, hard cuts, and query-param deep links.
