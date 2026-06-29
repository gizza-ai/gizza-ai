# tail-lines — competitor analysis & surface checks (2026-06-30)

**Tool:** `tail-lines` — output the last N lines of text, optionally skipping footer lines and prefixing original line numbers.

## Verification snapshot

Verified on 2026-06-30 (CARGO_BUILD_JOBS=1).

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/tail-lines && cargo test --workspace` | ✅ 14 passed (13 core + 1 schema drift guard; web 0) |
| Wafer block | `cd blocks/tail-lines && wafer build` | ✅ OK gizza-ai/tail-lines v0.1.0 (293.3 KiB) |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ 3/3 pass (default-ten, last-three, skip-and-number) |
| Web build | `wasm-pack build blocks/tail-lines/web --target web --release --out-dir pkg` | ✅ pkg built |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered tools/tail-lines/ (290 tools) |
| CLI | `gizza tool tail-lines ...` | ✅ returned `c\nd\ne` and numbered skip output `2\tx\n3\ty` |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-tail-lines.spec.ts` | ✅ 2/2 passed |

## Competitor scan

Representative tools and feature patterns:

1. **GNU/BSD `tail`** — last N lines, follow mode, byte mode, multiple files.
2. **TextFixer / browser text-line utilities** — keep/remove leading or trailing lines from pasted text.
3. **CyberChef Take lines / Drop lines-style recipes** — composable text slicing with start/end ranges.
4. **Log viewers** — show bottom log entries and optionally ignore footer/noise rows.
5. **Spreadsheet/text cleanup tools** — grab the bottom rows and sometimes retain source row numbers.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| Keep last N lines | Core `tail -n` behaviour | ✅ `count` parameter, default 10 |
| Handle fewer-than-N inputs | Common text utility behaviour | ✅ returns all available lines |
| Preserve trailing newline / CRLF | CLI parity expectation | ✅ core preserves trailing newline and CRLF line endings |
| Skip footer lines before taking tail | Useful for logs/reports | ✅ `skip` drops N lines from the end first |
| Prefix source line numbers | `nl`/editor/log-viewer pattern | ✅ `number=true` uses 1-based original line numbers |
| Follow live files / streaming | `tail -f` | Out of scope: gizza tools process a static text input |
| Byte-based tailing | `tail -c` | Out of scope for this text-line tool |
| Multiple files with headers | CLI `tail file...` | Out of scope for pasted text; users can run separate invocations |

## Notes

This complements `head-lines` rather than duplicating it: `head-lines` slices from the start, while `tail-lines` slices from the end and supports footer skipping. The page uses a textarea for multiline input, integer fields for `count`/`skip`, and a checkbox for numbering.
