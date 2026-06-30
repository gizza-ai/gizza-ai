# strip-ansi-codes — competitor analysis & surface checks (2026-06-29)

**Tool:** `strip-ansi-codes` — remove ANSI escape/color codes from terminal output, logs, and CI text.

## Verification snapshot

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/strip-ansi-codes && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 14 tests passed (core scanner, drift guard, web crate compile test) |
| Wafer block | `cd blocks/strip-ansi-codes && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` validated |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ `bad-scope`, `color-scope`, `osc-hyperlink`, and `strip-all` fixtures passed |
| Web build | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/strip-ansi-codes/web --target web --release --out-dir pkg` | ✅ web/pkg generated |
| Page generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/strip-ansi-codes/` |
| CLI | `CARGO_BUILD_JOBS=1 cargo install --path cli --force`; `gizza tool strip-ansi-codes ...` | ✅ returns clean text for SGR/control/OSC examples |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-strip-ansi-codes.spec.ts` | ✅ validates all-mode, color-only mode, and query-param deep-link |

## Competitor scan

Search query: `online strip ANSI escape codes remove terminal color codes tool`.

Representative sources and feature patterns:

1. **The Text Tool — Remove ANSI Codes** — browser text-area tool focused on stripping terminal ANSI color/control sequences from pasted text.
2. **Stack Overflow / Super User / Unix & Linux answers** — common command-line recipes use regex/sed/perl to strip CSI/SGR escape sequences; many recipes miss OSC or non-color control sequences.
3. **Log-cleaner snippets and packages** — developer utilities typically provide a simple “strip all ANSI” path for CI logs and captured command output.
4. **Terminal/color libraries (`strip-ansi`-style behavior)** — expected behavior is preserving printable Unicode while removing ESC-prefixed control bytes.
5. **ANSI color-code documentation/examples** — SGR (`ESC[...m`) is the most visible case, but cursor/erase (`ESC[2J`, `ESC[H`) and OSC strings (`ESC]...BEL` / `ESC]...ESC\`) also appear in real terminal streams.

## Gap analysis

| Capability / UX pattern | Competitors / references | Implemented in gizza |
| --- | --- | --- |
| Strip SGR colors/styles | Core behavior of most removers | ✅ `scope=all` and `scope=color` remove SGR (`CSI ... m`) |
| Strip all terminal escape sequences | Dedicated ANSI removers; many snippets attempt this | ✅ all mode removes CSI cursor/erase, OSC strings, DCS/SOS/PM/APC, and generic ESC sequences |
| Color-only mode | Useful when preserving terminal layout/control | ✅ `scope=color` keeps non-SGR control sequences while dropping color/style codes |
| OSC hyperlinks/window titles | Often missed by simple regex snippets | ✅ OSC BEL and ST terminators covered by scanner and tests |
| Unicode-safe cleaning | Expected for logs with emoji/non-ASCII | ✅ byte scanner removes only ASCII escape bytes and preserves UTF-8 text |
| Multiline paste / CI logs | Common UI requirement | ✅ page uses multiline text input; newlines preserved |
| Query-param deep-link | gizza page convention | ✅ Playwright covers text + scope deep-link |
| Server-side upload / accounts | Some web utilities may process remotely | Not needed: gizza runs locally in WASM/chat/CLI |

## Notes

The implementation intentionally uses a small ECMA-48-style scanner instead of a broad regex. This keeps the core dependency-free, wasm-safe, and able to handle OSC hyperlinks/window titles plus truncated sequences more predictably than common one-line shell snippets.
