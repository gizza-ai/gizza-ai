# latex-to-mathml — competitor analysis & surface checks (2026-06-29)

**Tool:** `latex-to-mathml` — convert a LaTeX math expression into a MathML `<math>` element. Pure Rust (`latex2mathml`), runs on chat block, CLI, and browser page.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + descriptor tests | `cd blocks/latex-to-mathml && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 8 core tests + 1 drift-guard schema test pass |
| Chat block (wasm32-wasip1) | `cd blocks/latex-to-mathml && CARGO_BUILD_JOBS=1 wafer build` | ✅ OK, `target/block.wasm` validates/instantiates (371.6 KiB) |
| Page wasm (wasm32-unknown-unknown) | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/latex-to-mathml/web --target web --release --out-dir pkg` | ✅ pkg built |
| CLI | `gizza tool latex-to-mathml latex='\\frac{1}{2}'`, inline + pretty options | ✅ compact block MathML and pretty inline MathML verified |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/latex-to-mathml/` |
| Page (Playwright) | `tool-page-latex-to-mathml.spec.ts` | ✅ 3 passed |

The chat/CLI schema is single-sourced from `descriptor()` and locked by the `schema_json_matches_authored_chat_schema` drift test.

## Competitor landscape

Top references users reach for:

1. **Temml** — JavaScript TeX-to-MathML converter with broad LaTeX coverage and public demo/tests.
2. **MathJax / Assistive MathML output** — rendering engine with extensive TeX macro coverage and accessibility-focused MathML output.
3. **LaTeXML** — NIST-backed LaTeX-to-XML/HTML/MathML system for documents and math, including presentation/content MathML workflows.
4. **Python `latex2mathml`** — library/CLI category reference for converting TeX math snippets to MathML.
5. **TeX4ht / authoring tutorials** — document-conversion workflow that can emit MathML as part of HTML output.

## Capability diff

| Capability | Competitors | gizza latex-to-mathml |
| --- | --- | --- |
| LaTeX math snippet → MathML | all | ✅ |
| Fractions, roots, powers/subscripts | all | ✅ |
| Greek letters and common symbols | all | ✅ |
| Block vs inline display | MathJax/Temml/LaTeXML | ✅ `display=block|inline` |
| Pretty-print markup | libraries/CLIs vary | ✅ optional one-element-per-line output |
| Browser-local/private conversion | Temml-style demo | ✅ wasm page, no upload |
| CLI + chat/LLM API | libraries/CLIs | ✅ `gizza tool` and chat block |
| Full LaTeX document conversion | LaTeXML/TeX4ht | ❌ out of model |
| Custom macro packages / semantic Content MathML | LaTeXML/MathJax ecosystem | ❌ out of model |
| Visual rendered preview | MathJax/Temml demos | ❌ page outputs source markup only |

## In-model gaps closed / confirmed

- Added a clear `latex` multiline input so users can paste multi-token expressions.
- Added `display` mode (`block` default, `inline` optional), matching common MathML embedding needs.
- Added optional pretty-printing for copy/edit workflows while preserving compact output by default.
- Added drift-guard schema coverage and Playwright page coverage for default block, inline superscript, and pretty output.
- Kept output as literal MathML source, which is the artifact users paste into HTML/EPUB/Office documents.

## Out-of-model (intentionally not built)

- **Full LaTeX document → HTML/XML/MathML conversion** — requires a much larger TeX/document engine and package model.
- **Arbitrary custom macro expansion / package loading** — needs a macro environment and compatibility layer beyond a focused local converter.
- **Content MathML / semantic enrichment** — specialized symbolic interpretation, not just presentation conversion.
- **Rendered preview** — would require a page-level math rendering integration; this tool intentionally outputs portable source markup.

No competitor copy, branding, or assets were used.
