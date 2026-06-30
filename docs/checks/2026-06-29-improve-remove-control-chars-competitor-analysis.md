# remove-control-chars — competitor analysis & surface checks (2026-06-29)

**Tool:** `remove-control-chars` — strip null bytes and non-printable Unicode control characters from text. Pure Rust, browser-local, CLI-compatible, and chat-block compatible.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core unit tests | `cargo test --workspace` in `blocks/remove-control-chars/` | ✅ 8 core + 1 drift-guard schema test pass |
| Chat block | `wafer build` in `blocks/remove-control-chars/` | ✅ OK, 289.1 KiB, instantiates |
| Page wasm | `wasm-pack build blocks/remove-control-chars/web --target web --release --out-dir pkg` | ✅ pkg built |
| CLI | `gizza tool remove-control-chars text=$'a\\ab\\177c'` and keep-flags/replacement smoke | ✅ returns cleaned text (`"abc"`, `"a_b_c"`) |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/remove-control-chars/` |
| Page | `xvfb-run npx playwright test tool-page-remove-control-chars.spec.ts` | ✅ 3 passed |

## Competitor landscape

Users typically solve this with one of these approaches:

1. **CyberChef recipes** — broad text/binary utility with operations such as removing null bytes, regular-expression replacement, and text cleanup. Powerful, but a large general-purpose workspace.
2. **Online non-printable character removers** — small paste-in/paste-out pages that delete ASCII control bytes or invisible characters. Fast, but definitions and privacy guarantees vary.
3. **Unix command-line snippets** (`tr -d`, `perl -pe`, `sed`) — flexible for developers, but require remembering control-character ranges and shell escaping.
4. **Text editors / IDEs** — find/replace with regex modes can remove specific ranges, but the workflow is manual and editor-specific.
5. **Language snippets** (Python/Ruby/JavaScript) — precise and scriptable, but not convenient for one-off cleanup.

## Capability diff

| Capability | Competitors | gizza remove-control-chars |
| --- | --- | --- |
| Remove null bytes | CyberChef, snippets, many web tools | ✅ |
| Remove all C0/C1/DEL control characters | CyberChef/snippets, some web tools | ✅ uses Rust `char::is_control()` |
| Preserve tabs by default | varies | ✅ configurable `keep_tabs` |
| Preserve line breaks by default | varies | ✅ configurable `keep_newlines` |
| Replace removed chars instead of deleting | regex/snippet tools, some web tools | ✅ `replacement` parameter |
| Unicode-safe text handling | varies | ✅ character-based pass, leaves printable Unicode intact |
| Browser-local privacy | CyberChef/offline pages | ✅ WebAssembly local execution |
| CLI and chat/API surface | snippets, CyberChef API alternatives | ✅ same descriptor-driven schema |

## In-model gaps closed / confirmed

The tool covers the high-value in-model cases: null-byte cleanup, C0/C1/DEL stripping, preserving meaningful whitespace by default, optionally removing tabs/newlines too, and replacing each removed character with a user-supplied string. The page copy explains the exact character class and privacy model without copying competitor branding or wording.

## Out-of-model / intentionally not built

- Visual highlighting of removed invisible characters: useful, but the current gizza text-output page model returns the cleaned text rather than a rich annotated diff.
- Binary file upload cleanup: this backlog item is text-oriented; file-byte transforms belong in a separate file tool.
- Full Unicode category filtering beyond Cc controls (e.g. format characters, zero-width joiners): related but semantically different from “control characters” and should be a separate invisible-character sanitizer if needed.

No competitor copy, branding, or trademarks were used.
