# plist-viewer competitor analysis (2026-06-29)

## Tool surface verified

- Chat/CLI: accepts XML plist text or Base64-encoded binary `bplist00` data, renders JSON or tree output, supports indent size, sorted keys, and Base64/hex rendering for `<data>` blobs.
- Page: `/tools/plist-viewer/` with multiline plist input, JSON/tree selector, indent field, sort checkbox, and data encoding selector.
- Privacy: parsing runs locally; plist contents are not uploaded.

## Competitors reviewed

1. **macOS `plutil -p` / `plutil -convert json`** — authoritative and handles XML/binary, but requires macOS and a terminal.
2. **Xcode property list editor** — convenient tree UI, but heavyweight and not script/copy friendly.
3. **VS Code plist extensions** — good developer workflow, but require local extension setup and vary by platform.
4. **Online plist viewers/converters** — easy paste/upload flows, but often upload potentially sensitive app/device config to a server.
5. **Python/Ruby plist libraries** — robust for automation, but users must write code and install tooling.

## Fit-to-model gaps and decisions

- Built in-model: XML plist parsing, Base64 binary plist parsing, pretty JSON, plutil-style tree output, stable sorted-key mode, data blob encoding choices, browser page, CLI/chat parity.
- Not built: drag-and-drop binary file upload, editable tree UI, diff/merge UI, schema validation against app-specific plist keys, or NSKeyedArchiver object graph reconstruction. Those are larger UI/domain tools; this tool stays a compact viewer/converter.
- Copy/branding: no competitor wording or proprietary UI was copied.

## Verification snapshot

- `cargo test --workspace` from `blocks/plist-viewer/`
- `wafer build` from `blocks/plist-viewer/`
- `wasm-pack build blocks/plist-viewer/web --target web --release --out-dir pkg`
- `cargo install --path cli`
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`
- `gizza tool plist-viewer ... format=tree`
- `cd tests && xvfb-run npx playwright test tool-page-plist-viewer.spec.ts`
- `npm run test`
