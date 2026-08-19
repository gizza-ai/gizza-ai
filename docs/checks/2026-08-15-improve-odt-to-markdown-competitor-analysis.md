# Competitor analysis: odt-to-markdown

Date: 2026-08-15
Tool: `odt-to-markdown`
Backlog request: Convert an OpenDocument Text (`.odt`) file into Markdown.

## Sources skimmed

Web search for "ODT to Markdown converter online features output format odt markdown" surfaced these real tools/services:

1. MConverter ODT to MD — browser upload / drag-and-drop flow with MD as the selected target format.
2. Zamzar ODT to MD — file upload, choose output format, convert/download flow.
3. Vertopal ODT to Markdown — file upload flow with an intermediate tools/options step before conversion.

A fourth result, `ikus060/odt2md`, is a small script wrapper around Pandoc rather than a browser-local converter; it is useful as a capability reference but not as the UX baseline for this gizza block.

## Table-stakes capabilities and decisions

| Capability / UX pattern | Competitor pattern | In model? | Decision for gizza |
| --- | --- | --- | --- |
| Accept `.odt` files | All conversion services center the flow on uploading an ODT file. | Yes | Use `Input::File` with URL/ref source resolution, matching other no-page document tools. |
| Markdown output | Services expose MD/Markdown as the target. | Yes | Default `format=markdown`, returning Markdown content in a flat JSON response. |
| Plain text fallback | General converters often offer adjacent text-like targets. | Yes | Add `format=text` enum for markup-stripped extraction when Markdown is not wanted. |
| Preserve headings and paragraphs | Expected from ODT-to-MD converters and Pandoc-style tools. | Yes | Render `text:h` using outline level and `text:p` as paragraphs. |
| Preserve emphasis and links | Common expectation for document conversion. | Yes | Render bold/italic styles and safe hyperlinks; unsafe schemes are emitted as text only. |
| Preserve lists | Common expectation for document conversion. | Yes | Render ordered/unordered/nested `text:list` structures. |
| Preserve simple tables | Markdown table output is a key quality differentiator for document converters. | Yes | Render ODF tables as GitHub-flavored Markdown tables; repeated empty filler cells are clamped. |
| Footnotes / images | Pandoc-like conversion keeps references and media markers where possible. | Partially | Render footnotes as Markdown footnotes and images as Markdown image references to their package paths; image binary extraction is out of scope for this text-output tool. |
| Batch uploads / cloud storage | MConverter/Zamzar-style services often support multi-file or cloud-provider flows. | Out of model | gizza blocks process one file per call and do not integrate with cloud accounts. |
| Visual conversion settings / download file | Hosted converters provide file-download UX. | Out of model for this block | This is a no-page chat+CLI tool returning JSON/text; standalone binary-file-to-text pages are not currently supported in this repo's generator. |
| High-fidelity round-trip layout | Full office layout fidelity needs a full ODF layout engine/Pandoc/LibreOffice. | Out of model | Document limits: focuses on readable Markdown text structure, not pixel/layout fidelity. |

## Implementation shape

The tool follows the proven `epub-to-markdown` no-page file-input pattern. It parses `.odt` as a ZIP container with `content.xml` plus optional `styles.xml`/`meta.xml`, and also accepts flat OpenDocument XML (`.fodt`) when the input is XML rather than ZIP. The conversion is pure Rust using `zip` and `quick-xml`, so it can run in the local wasm/wafer runtime without server-side LibreOffice or Pandoc.

Descriptor parameters:

- `format` enum: `markdown` (default) or `text`.

Output fields:

- optional `title`
- optional `creator`
- `format`
- `content`
- `chars`
- `paragraphs`
- `tables`
- `images`
- `truncated`

## Limits documented for users

- Converts readable ODT text structure, not exact office layout.
- Comments and tracked changes are omitted.
- Embedded images are referenced by package path in Markdown; image binaries are not extracted.
- Complex styling, text boxes, formulas, indexes, and page layout may degrade to plain readable text.
- Output is clipped at 2,000,000 characters to keep tool responses manageable.
