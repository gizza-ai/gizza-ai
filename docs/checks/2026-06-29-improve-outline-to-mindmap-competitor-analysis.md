# Outline to Mind Map competitor analysis (2026-06-29)

Tool: `outline-to-mindmap`

## Competitors reviewed

1. Mermaid mindmap syntax / Mermaid editors
   - Converts a text hierarchy into a mind map using Mermaid syntax.
   - Strong ecosystem and editable source, but users must know Mermaid-specific syntax rather than pasting a plain indented outline.
2. MindMap AI text-to-mindmap
   - AI-assisted text-to-mind-map generation from notes or documents.
   - Useful for unstructured text, but AI/cloud centric rather than deterministic local conversion.
3. MindLM text-to-mindmap
   - Generates editable maps from pasted notes with AI structuring.
   - Focuses on AI interpretation; not a simple offline outline-to-SVG transform.
4. AmyMind
   - AI-powered mind mapping from text, Markdown, and documents.
   - Polished editing/generation workflow; cloud/editor oriented.
5. Whimsical / Miro-style mind map makers
   - Rich interactive editors for manual mind-map creation and collaboration.
   - Excellent editing UX, but not a lightweight local CLI/chat/page converter.

## In-model gaps and actions taken

- Plain outline input: implemented indentation-based parsing where each line is a node, deeper spaces/tabs mean nesting, and common bullet/number markers are stripped.
- Deterministic local SVG: implemented a pure-Rust parser, simple tidy layout, and standalone SVG emitter that runs in chat, CLI, and browser page surfaces without uploading text.
- Layout choices: added `right` and `down` layouts to cover classic horizontal mind maps and top-down tree/org-chart diagrams.
- Visual controls: added branch coloring, monochrome mode, dark-mode rendering, and a configurable central title for multi-root outlines.
- Robustness: added XML escaping, long-label truncation, empty-input errors, node/depth limits, tab indentation handling, and core unit coverage.
- Page copy: documented outline format, options, example input, SVG use cases, and privacy/local execution.

## Out-of-model or intentionally not implemented

- Interactive drag-and-drop editing/collaboration: valuable but outside the generated static tool page model.
- AI summarization/structuring of arbitrary prose: out of model for this deterministic pure converter and would require an LLM/model backend.
- Mermaid syntax import/export: future extension; current scope is plain indented outline to SVG.
- PNG/PDF export buttons: SVG text is returned directly; users can save it or pass it to other gizza conversion tools.

## Verification snapshot

- `cargo test --workspace` from `blocks/outline-to-mindmap`: passed.
- `wafer build` from `blocks/outline-to-mindmap`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/outline-to-mindmap/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered `tools/outline-to-mindmap/`.
- `gizza tool outline-to-mindmap outline='...' direction=right colorful=true dark_mode=false title='Mind Map'`: passed.
- `cd tests && xvfb-run npx playwright test tool-page-outline-to-mindmap.spec.ts --timeout=120000 --reporter=line`: passed.
