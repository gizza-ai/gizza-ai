# markdown-section-extractor — competitor analysis & surface checks (2026-06-29)

**Tool:** `markdown-section-extractor` — extract one Markdown section by heading, with controls for nested subsections and heading inclusion.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/workspace tests | `cd blocks/markdown-section-extractor && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 18 tests passed (descriptor drift guard + parser/extraction cases) |
| Chat block | `cd blocks/markdown-section-extractor && CARGO_BUILD_JOBS=1 wafer build` | ✅ produced and validated `target/block.wasm` |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/markdown-section-extractor/web --target web --release --out-dir pkg` | ✅ built `web/pkg` |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/markdown-section-extractor/` |
| CLI | `gizza tool markdown-section-extractor markdown=$'...' heading=Install` and `include_heading=false` | ✅ returned the selected section and body-only output |
| Page | `cd tests && xvfb-run npx playwright test tool-page-markdown-section-extractor.spec.ts` | ✅ 5 passed (default extraction, checkboxes, contains mode, query-param deep-link) |

## Competitor scan

Searches reviewed:
- `online markdown section extractor extract heading tool competitors`
- `markdown heading extractor split markdown by heading online`

Representative competitors and references:

1. **Markdown Slicer** — browser tool for splitting large Markdown files; supports manual extraction and auto-splitting by heading level, client-side.
2. **Markdown Toolkit / Markdown Split** — splits large Markdown files by H1/H2 headings or custom delimiters and downloads ZIP output.
3. **LangChain MarkdownHeaderTextSplitter** — developer library that chunks Markdown by configured header levels for retrieval pipelines.
4. **Haystack heading extraction discussion** — developer ecosystem evidence that preserving headings during Markdown preprocessing is a common need.
5. **csplit / shell recipes for splitting Markdown by chapter** — command-line approach for splitting a document by headings.

## Gap / fit analysis

| Capability | Competitors | gizza `markdown-section-extractor` | Decision |
| --- | --- | --- | --- |
| Extract or split by heading | Slicers and splitter libraries split by heading hierarchy | ✅ extracts one section by heading text | Built |
| Nested subsection control | Splitters usually chunk by heading level | ✅ `include_subsections=true` keeps deeper headings; false stops at first child heading | Built |
| Include/omit heading | Useful for copy-paste and programmatic pipelines | ✅ `include_heading` checkbox/param | Built |
| Matching modes | Some tools require exact headings or select manually | ✅ exact case-insensitive, exact case-sensitive, contains | Built |
| Markdown heading forms | Many simplistic regex tools only handle `#` ATX headings | ✅ supports ATX and setext headings, strips ATX closing hashes | Built |
| Code-block safety | Regex splitters can mistake `#` in fenced code for headings | ✅ skips headings inside fenced code blocks and rejects indented code headings | Built |
| Batch split/download ZIP | Markdown Slicer / Markdown Toolkit can export many files | ❌ out-of-model for this single-output text tool; future tool could be a multi-section splitter | Not built |
| Full Markdown AST / CommonMark compliance | Libraries may parse more edge cases | Partial: focused no-dependency scanner for practical ATX/setext/fenced-code cases | Fit for pure wasm loop |

## Improvements made from analysis

- Added controls that matter for documentation/RAG workflows: nested subsection inclusion, body-only output, and substring matching.
- Implemented setext headings and fenced-code-block skipping to avoid the biggest regex-splitter mistakes.
- Added page tests for all user-facing controls and query-param deep-link behavior.
