# rtf-to-markdown competitor analysis (2026-08-08)

Tool: `rtf-to-markdown` — convert raw Rich Text Format source to Markdown or readable text.

## Sources scanned

Search query: `RTF to Markdown converter online options preserve headings lists tables links`.

Top relevant results reviewed from snippets/pages available in search:

1. Word Spinner RTF-to-Markdown workflow — positions the job as converting RTF, then cleaning headings, links, lists, and formatting.
2. Smart Markdown RTF to Markdown Converter — table stakes include clean GitHub-flavored Markdown, structural hierarchy, headings, paragraphs, lists, tables, and inline formatting.
3. JustMarkdown Text to Markdown — broader rich-text/TXT/HTML/RTF paste flow; mentions smart detection for headings, lists, links, and tables with no upload.
4. File2Markdown article/tool page — emphasizes preserving semantic structure while dropping visual styling such as fonts and colors.

## Table-stakes capabilities

| Capability / UX pattern | In current gizza model? | Decision |
| --- | --- | --- |
| Paste raw RTF source into a text area | Yes | Build as the required multiline `rtf` parameter. |
| Preserve headings | Yes | Detect `\\outlinelevelN` and stylesheet names such as `heading 1`; expose `headings=auto/off`. |
| Preserve bold/italic/strikethrough | Yes | Convert to `**bold**`, `*italic*`, and `~~strike~~`. |
| Handle underline | Yes, with caveat | Markdown has no underline syntax; expose `underline=html/ignore`, defaulting to inline `<u>`. |
| Preserve links | Yes | Convert RTF `HYPERLINK` fields to Markdown links; expose a boolean toggle. |
| Preserve lists | Yes | Detect common `\\listtext`/`\\pntext` markers and indentation levels. |
| Preserve tables | Yes, simplified | Emit GitHub pipe tables by default; expose `tables=text` for tab-separated rows. |
| Decode Unicode and Windows-1252 escapes | Yes | Decode `\\uN` and `\\'hh` sequences so smart quotes, accents, and symbols survive. |
| Copy/download text result | Platform-provided | `format = "text"` pages get the generic copy/download affordances. |
| File upload of `.rtf` files | Partly, but not necessary | The generic page model can paste text; file upload for raw text is not required for this pure text tool and would add browser file plumbing outside the scaffold pattern. |
| Preserve fonts/colors/images/layout/revisions | No | Out-of-model for Markdown or not representable; document as skipped metadata rather than pretending fidelity. |
| Convert `.doc`/`.docx` binaries | No | Different formats; out of scope for a pure Rust RTF parser in this block. |

## Defaults chosen

- `headings=auto` — competitors emphasize structural hierarchy, so heading detection should be on by default.
- `tables=markdown` — Markdown pipe tables match user expectation for a Markdown converter.
- `underline=html` — preserve the signal where Markdown lacks syntax; users can choose pure text via `ignore`.
- `links=true` — links are a table-stakes conversion feature.
- `escape_markdown=true` — literal punctuation from RTF should not accidentally become Markdown syntax.

## Examples and controls

Competitor UX is mostly paste-and-convert with simple output. This tool adds preset chips for a heading/emphasis example, a Markdown table example, and a tab-separated table example. Enum controls use friendly labels for headings, tables, and underline; booleans render as checkboxes through the generated manifest.
