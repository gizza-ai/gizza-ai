# smart-quotes-clean — competitor analysis & surface checks (2026-06-29)

**Tool:** `smart-quotes-clean` — replace smart quotes, dashes, ellipses, prime marks, guillemets, exotic spaces, and zero-width characters with plain ASCII-safe text while preserving ordinary Unicode.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + drift tests | `cd blocks/smart-quotes-clean && cargo test --workspace` | Covers quotes, dashes, ellipsis, prime marks, guillemets, space normalization, Unicode preservation, and descriptor schema drift. |
| Chat block | `cd blocks/smart-quotes-clean && wafer build` | Validates the wasm32-wasip1 block. |
| Page wasm | `wasm-pack build blocks/smart-quotes-clean/web --target web --release --out-dir pkg` | Builds browser wrapper. |
| CLI | `gizza tool smart-quotes-clean text=…` | Verifies direct CLI output. |
| Page | `xvfb-run npx playwright test tool-page-smart-quotes-clean.spec.ts` | Verifies live page, options, checkbox, and query-param deep links. |

## Competitor landscape

1. **TextFixer / Smart Quotes Converter-style pages** — usually convert curly quotes to straight quotes, but many ignore dashes, ellipses, prime marks, guillemets, or invisible spaces.
2. **Online ASCII cleaners / Unicode punctuation removers** — often over-normalize by stripping accents or all non-ASCII characters, which is unsafe for names, CJK text, and emoji.
3. **Word processor preferences (“smart quotes” off)** — prevent future substitutions but do not clean existing pasted text.
4. **Regex snippets / `sed` recipes** — powerful but brittle; users must remember Unicode codepoints and often miss zero-width/non-breaking-space characters.

## Capability diff

| Capability | Competitors | gizza smart-quotes-clean |
| --- | --- | --- |
| Curly double/single quotes → ASCII | common | ✅ |
| En dash/minus/nonbreaking hyphen → `-` | some | ✅ |
| Configurable em dash rendering | rare | ✅ `--`, `-`, or ` - ` |
| Ellipsis glyph → `...` | some | ✅ |
| Prime marks / guillemets | uncommon | ✅ |
| NBSP/thin/ideographic spaces → ASCII space | uncommon | ✅ optional, default on |
| Zero-width chars / BOM removal | uncommon | ✅ optional, default on |
| Preserve accents/CJK/emoji | mixed | ✅ leaves ordinary Unicode intact |
| Private browser-local processing | mixed | ✅ WASM page + CLI + chat |

## In-model improvements included

- A small deterministic pure-Rust mapping that avoids heavy Unicode transliteration and preserves user content.
- `em_dash` selector for the three common plain-text conventions.
- `normalize_spaces` checkbox for users who need to preserve exact whitespace.
- Live page with query-param support for pasteable cleanup links.

## Out-of-model / intentionally not built

- Language-aware typography repair or grammar correction; this tool only normalizes common typographic characters.
- Full transliteration to ASCII (e.g. accents or CJK romanization); that would be a separate tool and risks data loss.
- Rich document cleanup for PDFs/DOCX; this is a text surface.

No competitor copy, branding, or trademarks were used.
