# Improve toc-generator — competitor analysis (2026-06-30)

## Scope

Tool: `toc-generator`

Goal: build linked tables of contents from Markdown or HTML headings, with Markdown/HTML output, level filtering, ordered/bulleted lists, GitHub-style anchors, duplicate disambiguation, and existing HTML `id` preservation.

## Competitor scan

1. GitHub / GitLab automatic heading anchors
   - Strengths: predictable anchors in rendered Markdown.
   - Gaps closed here: generates the actual TOC text to paste into a README, supports level filtering and ordered lists, and works before publishing.

2. VS Code Markdown All in One TOC command
   - Strengths: editor-integrated Markdown TOC generation.
   - Gaps closed here: browser/CLI/chat availability, HTML input support, no editor extension required, and machine-testable output.

3. doctoc / markdown-toc npm tools
   - Strengths: mature Markdown TOC generators for repositories.
   - Gaps closed here: no install needed for casual use, explicit HTML output mode, and single-file local page use.

4. Online Markdown TOC generators
   - Strengths: quick paste-and-copy workflow.
   - Gaps closed here: runs locally in the browser, supports HTML headings with existing ids, and exposes the same core via gizza CLI/chat.

5. Static-site generators' TOC components
   - Strengths: integrated with rendered pages and themes.
   - Gaps closed here: standalone conversion for arbitrary documents and predictable copied Markdown/HTML snippets.

## In-model improvements included

- Markdown ATX and setext heading extraction.
- HTML `<h1>`…`<h6>` extraction with existing `id` preservation.
- Ignores Markdown headings inside fenced code blocks.
- Strips inline Markdown/HTML formatting from display text.
- GitHub-style slug generation with duplicate `-1`, `-2` suffixes.
- `min_level` / `max_level` filtering and ordered-list option.
- Markdown and HTML TOC output formats.
- SEO/help copy for README, wiki, blog, and long-document use cases.

## Out-of-model / not built

- Updating an existing TOC region in a source file, because this tool returns a generated snippet rather than editing files in-place.
- Renderer-specific anchor algorithms for every static-site generator. The default targets GitHub-style anchors and preserves explicit HTML ids.

## Verification checklist

- Core unit tests cover Markdown headings, duplicate anchors, level filtering, ordered lists, inline formatting, code fences, setext headings, HTML ids, HTML output, skipped levels, and error cases.
- Drift-guard schema test covers the chat/LLM descriptor.
- Web wrapper exposes `run(document, input_format, output_format, min_level, max_level, ordered)`.
- Playwright tests cover Markdown page output, HTML page output, and query-param deep links.
