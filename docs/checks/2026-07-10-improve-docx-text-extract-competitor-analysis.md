# docx-text-extract — competitor analysis (2026-07-10)

Scan performed before verification. One web search: "online docx to markdown text extractor convert Word document to markdown plain text". Notes are paraphrased; no competitor wording, branding, examples, or trademarks are copied into the tool.

## Competitors skimmed

1. **word2md.com** — browser-local Word/Google Docs to GitHub-Flavored Markdown conversion, oriented around clean Markdown output.
2. **SiteGPT DOCX to Markdown** — upload a Word file and convert it to Markdown for documentation/content migration.
3. **word2md.net** — browser-side DOCX to Markdown, emphasizing structured Markdown output.
4. **Monkt Word to Markdown** — converts DOCX/DOC-style documents to Markdown for content workflows and AI ingestion.
5. **CloudConvert DOCX to MD** — cloud conversion from Word document to Markdown file.

## Table-stakes parameters / features

| Feature | Competitors | In-model here? | Decision |
|---|---|---|---|
| Accept `.docx` upload or URL/ref | all | ✅ | `url`/`ref` document source; DOCX ZIP/OOXML validated in core |
| Plain text extraction | text-extractor tools | ✅ | `format=text` returns flattened paragraphs/tables |
| Markdown conversion | word-to-md tools | ✅ | `format=markdown` or `both` returns GFM-like Markdown |
| Headings from Word styles | Markdown converters | ✅ | heading styles/Title become `#` headings |
| Bullet/numbered lists | Markdown converters | ✅ | numbering.xml maps decimal vs bullet lists |
| Tables | Markdown converters | ✅ | Word tables render as Markdown pipe tables |
| Hyperlinks | Markdown converters | ✅ | relationship targets become Markdown links |
| Bold/italic runs | Markdown converters | ✅ | inline emphasis preserved |
| Output selection | converter tools | ✅ | `format` enum: both/markdown/text |
| Legacy `.doc` support | some cloud converters | ⚠️ out-of-model | `.doc` is a different binary format; this tool is DOCX/OOXML only. |
| OCR of scanned pages/images | document AI tools | ⚠️ out-of-model | Requires OCR/ML; this reads the DOCX text layer and ignores embedded images. |
| Full styling/layout fidelity | office converters | ⚠️ out-of-model | Markdown/plain text intentionally drop fonts, colors, page layout, headers/footers. |
| Downloadable `.md` file page | web converters | ⚠️ out-of-model for this block | This is a chat+CLI document block; file-download page is not present for binary DOCX input. |

## Design decisions

- **DOCX-specific structure extraction.** Existing generic document extractors flatten text; this block reconstructs useful Markdown structure for headings, lists, tables, links, and emphasis.
- **No external office engine.** The implementation parses the DOCX ZIP/WordprocessingML with pure Rust crates (`zip`, `quick-xml`) so it can instantiate under wafer/wasmi.
- **Explicit `format` selector.** Competitors often return just Markdown; this tool supports Markdown, text, or both from the same parse.
- **Clear limits.** It supports `.docx` (not legacy `.doc`), text-layer content (not OCR), and structural Markdown rather than pixel-perfect layout.
