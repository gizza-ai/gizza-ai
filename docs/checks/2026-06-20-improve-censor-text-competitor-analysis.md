# censor-text — competitor analysis (2026-06-20)

Twenty-second `/create-next-tool` backlog pick (burn-subtitles skiplisted before
it — needs video+srt = 2 inputs). Pure-Rust (no deps) text tool, all 3 surfaces.
Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| profanity filters / redaction tools | mask a word list or built-in profanity; choose mask char | capabilities |
| text-redaction utilities | whole-word vs substring; case-insensitive; keep length | capabilities |

## Gap diff vs our tool
Our tool: redact a comma-separated `words` list (or a built-in common list) by
masking each match with a chosen `mask` char; case-insensitive; whole-word by
default (so `ass` won't hit `class`) with a substring mode; length-preserving;
Unicode-safe for unmasked text. Covers the standard feature set.

**In-model gaps considered, deferred (minor):**
- **Partial reveal** (keep first/last char, e.g. `d**n`) — a masking-style option.
- **Regex patterns** (mask emails/phones by pattern) — would add the `regex` crate;
  a more advanced redaction mode / sibling tool.
- **Per-word mask length** (fixed `[redacted]` token instead of repeating a char).

**Out-of-model:** ML toxicity classification (needs a model — different tool).

## Tested
unit (8: masks supplied words case-insensitive, whole-word skips substrings,
substring mode masks inside words, default list when empty, custom mask char,
multiple words, empty-text error, unicode preserved) + drift-guard · wafer
fixtures (1) · `wafer build` · wasm-pack web · generator · CLI (masks 'damn',
leaves 'ass' inside 'class'/'assignment') · Playwright page + query deep-link (2).

> Original work only — no competitor copy, branding, or trademarks copied.
