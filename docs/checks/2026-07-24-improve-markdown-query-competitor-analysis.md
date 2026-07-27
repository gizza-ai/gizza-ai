# markdown-query competitor analysis (2026-07-24)

Tool: `gizza-ai/markdown-query` — extract selected structures from Markdown into text, JSON, or reconstructed Markdown.

## Competitor scan

| Source | Positioning | Table-stakes observed |
| --- | --- | --- |
| Markdown link / heading extractor utilities | Paste Markdown and quickly list one specific element family. | Large textarea input, selector for element type, simple text output, copy-friendly results. |
| Markdown parsers and AST explorers | Developer-focused inspection of Markdown syntax trees. | CommonMark/GFM behavior, structured JSON output, code fence language preservation, table parsing. |
| Documentation linting / TOC helper tools | Audit docs and build navigation from headings or links. | Include source positions/line numbers, headings with level, links with labels and URLs, predictable output for scripts. |

## Fit-to-model decisions

| Capability / UX pattern | Decision | Rationale |
| --- | --- | --- |
| Headings extraction | Built | Needed for table-of-contents and docs auditing workflows. |
| Links extraction | Built | Core use case; returns link text, destination, and title metadata when available. |
| Images extraction | Built | Images use similar Markdown syntax but are distinct from normal links, so they are a separate mode. |
| Code block extraction | Built | Preserves fenced language tags and block content for docs/code audits. |
| GitHub-style table extraction | Built | GFM pipe tables are common in README files; table alignment is preserved in Markdown output. |
| Text output | Built | Fast copy/paste list for humans. |
| JSON output | Built | Scriptable `{ count, items }` shape for downstream processing. |
| Markdown output | Built | Reconstructed snippets can be pasted into another Markdown document. |
| Source line numbers | Built | Table-stakes for linting and source navigation. |
| Full Markdown AST query language | Out-of-model for this focused tool | Powerful but much larger than the simple extractor UX; better as a future AST-query tool. |
| Remote URL fetching / repository crawl | Out-of-model | The public tool model here is local browser/CLI execution; batch crawling belongs in a network or repo-analysis tool. |
| Automatic link checking | Out-of-model | Requires network access and has a different failure/timeout model; this tool only extracts links. |

## Verification plan

- Unit tests cover links, headings with line numbers, images separated from links, fenced code blocks, tables, JSON output, Markdown output, and empty-input errors.
- CLI checks should assert exact output for links and headings.
- Page checks should assert exact rendered output and a query-param deep link.
- Hygiene should enforce page placeholders, FAQ accordions, manifest sync, and no brand strings.
