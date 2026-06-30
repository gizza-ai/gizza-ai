# text-diff — competitor analysis & surface checks (2026-06-30)

**Tool:** `text-diff` — compare two text blocks and highlight added, removed, and changed lines as unified diff text or structured JSON.

## Verification snapshot

Verified on 2026-06-30 (CARGO_BUILD_JOBS=1).

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/text-diff && cargo test --workspace` | ✅ 10 passed (9 core + 1 schema drift guard; web 0) |
| Wafer block | `cd blocks/text-diff && wafer build` | ✅ OK gizza-ai/text-diff v0.1.0 (349.4 KiB) |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ 2/2 pass (unified, json-ignore) |
| Web build | `wasm-pack build blocks/text-diff/web --target web --release --out-dir pkg` | ✅ pkg built |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered tools/text-diff/ (291 tools) |
| CLI | `gizza tool text-diff ...` | ✅ unified diff and JSON ignore-case/whitespace paths returned expected output |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-text-diff.spec.ts` | ✅ 2/2 passed |

## Competitor scan

Representative tools and feature patterns:

1. **Diffchecker / Text Compare** — paste two blocks, see additions/removals, ignore whitespace/case options.
2. **GNU `diff` / git diff** — unified diff format, context lines, line-oriented comparison.
3. **Meld / Beyond Compare** — visual side-by-side comparison and change pairing.
4. **CyberChef Diff operation** — in-browser text comparison workflows.
5. **Online JSON/text compare utilities** — structured reports and machine-readable output for automation.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| Line-level diff | Common | ✅ LCS line diff |
| Unified diff output | CLI/git tools | ✅ `format=unified` with context hunks |
| Structured report | APIs/automation tools | ✅ `format=json` with counts and line operations |
| Changed-line pairing | Visual diff tools | ✅ adjacent removed/inserted blocks are counted as changed pairs |
| Ignore case | Common compare option | ✅ `ignore_case` |
| Ignore whitespace | Common compare option | ✅ whitespace normalization for matching |
| Side-by-side visual UI | Visual diff editors | Out of scope: current page renders text output, not a two-pane diff viewer |
| Character-level intra-line highlights | Advanced diff viewers | Out of scope: line-level tool optimized for simple pasted text and CLI output |

## Notes

The comparison preserves original line text in output even when case or whitespace normalization is enabled for matching. CRLF and LF line endings compare cleanly by trimming a trailing carriage return per line.
