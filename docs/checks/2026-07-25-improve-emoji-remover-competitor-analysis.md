# emoji-remover — competitor analysis (2026-07-25)

Scan done before implementation. Search query: "emoji remover online strip emoji from text". Reviewed the top reachable browser-style tools and paraphrased findings only; no competitor marketing copy or branding is reused in product text.

## Competitors reviewed

1. General text-cleaning emoji remover tools: paste text, click remove, get cleaned text. Most have a single textarea, no upload, and a copy button.
2. Developer-oriented emoji strip snippets and utilities: emphasize Unicode emoji coverage, including flags, skin tones, ZWJ families, and variation selectors.
3. Social-media caption cleaners: commonly offer either delete emoji entirely or replace removed emoji with spaces so words do not accidentally concatenate.

## Table-stakes parameters and model fit

| Capability | Typical default | In-model? | Decision |
|---|---|---|---|
| Text input textarea | empty | yes | `text` required multiline input |
| Delete emoji entirely | default | yes | `mode=remove` |
| Leave a space where each emoji was removed | optional | yes | `mode=space` |
| Custom placeholder per emoji | sometimes offered | yes | `mode=placeholder` + `placeholder` text |
| Collapse extra whitespace after removal | often expected | yes | `collapse_whitespace` boolean, default false to preserve exact text unless requested |
| Keep text-default symbols such as ©, ®, ™, bare hearts | advanced Unicode control | yes | `keep_text_symbols` boolean, default false for aggressive stripping |
| Handle flags, skin tones, keycaps, and ZWJ families as whole units | expected for correctness | yes | grapheme-cluster based core with Unicode ranges |
| Batch files / document upload | uncommon for simple tools | out-of-model | single text surface only |
| AI sentiment/content rewriting | not needed | out-of-model | not a pure deterministic text-cleaning function |

## UX decisions

- Use a textarea for the source text.
- Use an enum/select for replacement mode so CLI, chat schema, and page cannot drift on accepted values.
- Use a regular text input for placeholder text.
- Use booleans for whitespace cleanup and text-symbol preservation.
- Include examples for plain removal and placeholder mode, plus page tests for deep links and non-default checkbox state.

## Correctness notes

Rust `regex` does not provide a portable one-token `\p{Emoji}`/`\p{Extended_Pictographic}` solution that also handles grapheme clusters and ZWJ sequences safely. The implementation therefore iterates Unicode extended grapheme clusters with `unicode-segmentation`, then classifies each cluster by emoji/pictographic ranges, regional indicators, keycaps, skin-tone modifiers, and variation selector behavior.
