# OPML Converter competitor analysis (2026-06-29)

Tool: `opml-converter`

## Competitors reviewed

1. convert.guru OPML Converter (`convert.guru/opml-converter`)
   - Converts OPML files to CSV, TXT, JSON, and PDF.
   - Focuses on uploaded files and broad output formats.
2. Code Beautify OPML Viewer (`codebeautify.org/opmlviewer`)
   - Viewer/editor with beautify/minify/tree display and CSV conversion.
   - Strong XML viewing UX; less focused on round-tripping OPML subscription folders.
3. Code Amaze OPML Viewer (`codeamaze.com/web-viewer/opml-viewer`)
   - Beautify/format/validate/minify OPML and convert to JSON/YAML/CSV/TSV.
   - Broad format list, but primarily a viewer/formatter surface.
4. BeautifyTools OPML to JSON (`beautifytools.com/opml-to-json-converter.php`)
   - OPML-to-JSON only; accepts pasted input, file upload, or URL.
   - Single-direction conversion.
5. Vertopal OPML converter (`vertopal.com/en/convert/opml`)
   - General document-conversion workflow built around uploading OPML and selecting output formats.
   - Useful breadth, but upload-centric.

## In-model gaps and actions taken

- Multi-direction conversion: implemented OPML, JSON, and CSV as both input and output formats, so users can do OPML ⇄ JSON ⇄ CSV in one local tool rather than using single-purpose converters.
- Folder/category preservation: CSV export includes a `category` column using the OPML folder path, and CSV import rebuilds nested folder outlines.
- Faithful JSON tree: JSON output preserves outline attributes and nested `outlines` arrays, so OPML ⇄ JSON round-trips the subscription tree instead of flattening everything.
- Privacy/local execution: implementation is pure Rust/WASM and runs locally in chat, CLI, and browser page surfaces; no upload flow is required.
- Readability controls: added a `pretty` option for indented OPML/JSON and compact output where needed.
- Page copy: documented conversion modes, folder round-trip behavior, spreadsheet-friendly CSV editing, and privacy/local execution.

## Out-of-model or intentionally not implemented

- OPML editing tree UI: useful competitor feature, but outside the current simple generated-page model.
- URL fetch input: competitors can load OPML by URL; the browser page intentionally keeps conversion local and paste-based, while CLI URL fetching is handled by the gizza source layer where appropriate.
- PDF/TXT/YAML/TSV output: possible future formats, but the selected backlog scope was OPML to/from JSON and CSV. YAML/TSV are not necessary to satisfy the main RSS/podcast subscription migration use case.

## Verification snapshot

- `cargo test --workspace` from `blocks/opml-converter`: passed.
- `wafer build` from `blocks/opml-converter`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/opml-converter/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered `tools/opml-converter/`.
- `gizza tool opml-converter input='<opml ...>' from=opml to=json pretty=false`: passed.
- `cd tests && xvfb-run npx playwright test tool-page-opml-converter.spec.ts --timeout=120000 --reporter=line`: passed (5 tests).
