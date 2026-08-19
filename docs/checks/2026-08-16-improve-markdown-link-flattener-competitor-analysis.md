# markdown-link-flattener competitor analysis (2026-08-16)

## Scope

Tool: take Markdown prose and remove inline link syntax while preserving useful text. The common output choices are visible text only, visible text with the URL in parentheses, or URL-only extraction. Images, reference definitions and code examples need explicit policy controls so documentation is not silently corrupted.

Research done 2026-08-16: web search for Markdown link removers, Markdown-to-text converters and link extractors, plus comparison with the behavior of common Markdown renderers. Findings are paraphrased; no competitor copy, branding or trademarks are reused in page copy.

## Competitor scan

| Source | Table stakes found | In-model decision |
| --- | --- | --- |
| Markdown-to-plain-text converters | Paste Markdown, strip formatting, keep readable text. Links typically become only their labels, and code blocks are often preserved as text. | Built the focused link-only version: `link_mode=text` is the default and preserves surrounding Markdown that is not link syntax. Code spans/fences are preserved by default. |
| URL/link extractor tools | Extract destinations from Markdown or HTML so the output can be audited, checked or pasted into a list. | Built `link_mode=url` for inline Markdown links. Image destinations are controlled separately via `image_mode=alt_url` or `drop`; reference-style resolution is listed as out of model rather than guessed. |
| Documentation cleanup snippets/scripts | Teams often want `text (url)` so a plain-text export still carries citations. Scripts usually use regexes and may flatten code examples accidentally. | Built `link_mode=text_url` and `preserve_code=true` by default. Link titles are ignored; destination text is the stable data users need. |
| Markdown renderers / CommonMark behavior | Full Markdown parsing handles nested label brackets, inline destinations with optional titles, images, reference definitions, and code spans/fences. | Built the in-model subset: inline links/images, nested brackets in labels, destinations wrapped in `<...>`, optional titles, code preservation, and reference-definition keep/drop. Full reference resolution is out of model for this small deterministic block. |
| Batch text-cleaning tools | Useful extras include large paste support, no network calls, copy/download output and clear error handling for malformed input. | Built 1,000,000-byte cap, browser-local deterministic execution, malformed links left unchanged, and page copy stating the edge cases. Copy/download is provided by the generic page shell. |

## Parameters and defaults

| Capability | Default / options | Status |
| --- | --- | --- |
| Source Markdown | Required `markdown`, up to 1,000,000 bytes | In model, built. |
| Inline link rewrite | `link_mode=text`; alternatives `text_url`, `url` | In model, built as enum. |
| Image handling | `image_mode=alt_text`; alternatives `alt_url`, `drop`, `keep_markdown` | In model, built as enum. |
| Reference definition lines | `reference_definitions=drop`; alternative `keep` | In model, built. Definition uses can remain visible as `[label][id]`. |
| Code protection | `preserve_code=true` | In model, built for backtick spans and fenced code blocks. |
| Link-title preservation | — | Out of model. Titles are presentation hints, not visible text; retaining them would surprise the plain-text/citation workflows. |
| Full reference-style link resolution | — | Out of model for this release. Correct resolution needs a full document reference map and duplicate-label rules. Definitions can be kept for manual review. |
| HTML links or Markdown rendering | — | Out of model. This tool is scoped to Markdown inline link syntax; existing HTML/text tools cover other formats. |
| URL validation/fetching | — | Out of model. The tool never contacts the network. |

## UX decisions taken from the scan

- Keep the default conservative: visible text only, image alt text only, drop dead reference definitions, preserve code examples.
- Use selects with concrete examples in labels; the difference between `text`, `text_url` and `url` is a workflow decision, not an implementation detail.
- Treat malformed links as ordinary text. A cleanup tool should never delete text just because a closing parenthesis was missing.
- Make image behavior explicit. A plain-text article and a media inventory want opposite answers for `![alt](path)`.
- State the reference-link limitation on the page and in FAQ instead of silently pretending `[label][id]` was resolved.
