# unicode-to-text — competitor analysis & surface checks (2026-06-30)

**Tool:** `unicode-to-text` — decode escaped Unicode notation back into readable text. Pure Rust, text-in/text-out, runs on chat block, CLI, and browser page.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + schema tests | `cd blocks/unicode-to-text && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 12 core tests + 1 drift-guard schema test pass |
| Chat block | `cd blocks/unicode-to-text && CARGO_BUILD_JOBS=1 wafer build` | ✅ OK, `target/block.wasm` validates (290.7 KiB) |
| Page wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/unicode-to-text/web --target web --release --out-dir pkg` | ✅ pkg built |
| Generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/unicode-to-text/` |
| CLI | `gizza tool unicode-to-text 'text=caf\\u00e9 \\u{1F600} U+2764 &#65;'` | ✅ returned `"café 😀 ❤ A"` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-unicode-to-text.spec.ts` | ✅ 4 passed |

## Competitor landscape

Top comparable tools and features users expect:

1. **Online Unicode tools / Browserling Unicode unescaper** — simple paste box for `\uXXXX` and HTML numeric references, with instant output.
2. **CyberChef From Charcode / HTML Entity decode recipes** — flexible decoding, supports multiple encodings but requires choosing operations.
3. **FreeFormatter / Code Beautify Unicode unescape utilities** — decode JavaScript/JSON-style Unicode escape strings, often focused only on `\uXXXX`.
4. **Python / JavaScript REPL snippets** (`unicode_escape`, `JSON.parse`) — powerful for developers but brittle and unsafe for mixed notations.
5. **HTML entity decoders** — handle `&#...;` and `&#x...;` but not code-point or backslash escape syntaxes.

## Capability diff

| Capability | Competitors | gizza unicode-to-text |
| --- | --- | --- |
| `\uXXXX` decoding | common | ✅ |
| UTF-16 surrogate pair combination | varies | ✅ |
| `\u{...}` braced escapes | uncommon | ✅ |
| `\xXX` byte/Latin-1 escapes | some | ✅ |
| `\U00000000` Python wide escapes | uncommon | ✅ |
| `U+XXXX` code-point notation | some | ✅ |
| HTML decimal `&#DDDD;` | common in entity decoders | ✅ |
| HTML hex `&#xHHHH;` | common in entity decoders | ✅ |
| Mixed notation in one input | often requires multiple tools/recipes | ✅ one pass |
| Invalid scalar handling | varies | ✅ replacement character, no panic |
| Local/private execution | varies | ✅ browser + CLI + chat block |

## In-model gaps closed / confirmed

The useful in-model competitor capabilities for a stateless decoder are covered: common JavaScript/JSON escapes, HTML numeric references, code-point notation, Python/Rust variants, surrogate pairs, mixed input, and pass-through for non-escape text. The page keeps a single multiline input so users do not need to classify the source notation first.

## Out-of-model / intentionally not built

- Full JavaScript string unescaping (`\n`, `\t`, octal escapes, quotes) is intentionally left to a separate string-unescape tool; this tool only decodes Unicode code-point notations.
- Named HTML entities like `&amp;` / `&eacute;` are a broader HTML entity decoder task; this backlog item specifically targets Unicode/numeric escapes.
- Charset transcoding or mojibake repair requires byte-level encoding detection and is out of scope for this pure text transformer.

No competitor copy, branding, or trademarks were used.
