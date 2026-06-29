# text-to-table — competitor analysis & surface checks (2026-06-30)

**Tool:** `text-to-table` — render CSV/TSV/custom-delimited text as an aligned ASCII grid or padded Markdown table.

## Verification snapshot

Verified on 2026-06-30 (CARGO_BUILD_JOBS=1).

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/text-to-table && cargo test --workspace` | ✅ 12 passed (11 core + 1 schema drift guard; web 0) |
| Wafer block | `cd blocks/text-to-table && wafer build` | ✅ OK gizza-ai/text-to-table v0.1.0 (336.2 KiB) |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ 2/2 pass (ascii, markdown-tab) |
| Web build | `wasm-pack build blocks/text-to-table/web --target web --release --out-dir pkg` | ✅ pkg built |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered tools/text-to-table/ (292 tools) |
| CLI | `gizza tool text-to-table ...` | ✅ ASCII and Markdown/TSV paths returned expected aligned output |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-text-to-table.spec.ts` | ✅ 2/2 passed |

## Competitor scan

Representative tools and feature patterns:

1. **csvkit / miller / terminal table tools** — convert delimited data to aligned terminal tables.
2. **Markdown table generators** — paste CSV/TSV and get GitHub-style pipe tables.
3. **ASCII table generators** — padded box/grid output for README files, issues, and plain-text reports.
4. **CyberChef CSV operations** — parse CSV and transform it into other text layouts.
5. **Spreadsheet copy/paste utilities** — quick TSV/CSV to presentable table conversion.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| CSV/TSV/custom delimiter parsing | Common | ✅ Rust `csv` parser with comma/tab/semicolon/pipe/space/single-char delimiters |
| Aligned ASCII grid output | Terminal/ASCII tools | ✅ `format=ascii` with padded cells and borders |
| Markdown pipe table output | Markdown generators | ✅ `format=markdown` with alignment marker row |
| Header handling | Common | ✅ first row as header or generated Column N headers |
| Alignment controls | Table formatters | ✅ left/right/center padding |
| Quoted CSV fields | CSV-aware tools | ✅ handled by `csv` crate |
| Spreadsheet formulas / calculations | Spreadsheet apps | Out of scope: formatting only |
| HTML/LaTeX output | Other converters | Existing tools cover CSV/LaTeX-ish paths; this tool focuses on ASCII/Markdown presentation |

## Notes

This intentionally complements `csv-to-table`: that tool targets Markdown/HTML conversion, while this tool adds padded ASCII grid output and explicit alignment for terminal/docs copy-paste workflows.
