# html-sanitizer competitor analysis (2026-07-31)

Backlog row: `html-sanitizer` — strips scripts, styles, and unsafe tags from pasted HTML to produce safe, clean markup or plain text.

Search query: "HTML sanitizer online remove scripts unsafe tags plain text safe HTML options".

## Competitor scan

| Tool | Observed table-stakes | UX/control patterns | Fit decision |
| --- | --- | --- | --- |
| Softbaba HTML Sanitizer / Safe HTML Cleaner | Paste HTML, remove dangerous tags, remove unsafe attributes such as event handlers, keep cleaned safe markup. | Large paste box, one-click sanitize, copy result, security-oriented examples. | In-model: safe-html mode, script/style removal, event-handler removal, URL scheme filtering, copyable text output. |
| ToolSura HTML Sanitizer / XSS Filter | Parse incoming HTML and rebuild from safe rules; focus on XSS filtering and allowed tags/attributes. | Paste box with sanitized result; explanatory copy about allowlists and unsafe tags. | In-model: allowlist behavior, dropped unsafe tags/attrs, documented limits. Out-of-model: browser-grade parsing parity with DOMPurify is not claimed. |
| HTML tag stripper / HTML cleaner tools | Convert HTML to clean plain text; often preserve line breaks, remove scripts/styles, copy/export text. | Mode-like choice between cleaned markup and stripped text; examples for pasted rich text. | In-model: plain-text mode after sanitization; examples for rich text. Out-of-model: URL fetch/bulk batch import is not part of this local pure block. |

## Decisions implemented

- Output mode enum: `safe-html` (default) and `plain-text`.
- Required multiline `html` input with example chips for XSS removal, plain text, lean CMS markup, and safe styles.
- Boolean controls:
  - `allow_links` default true: safe URL schemes and relative URLs survive; script/data-text URLs do not.
  - `allow_images` default true: safe `<img>` tags can survive; disabling removes images.
  - `allow_styles` default false: inline styles are removed unless explicitly enabled; obvious script vectors are still blocked.
  - `keep_classes` default true: can be disabled to remove pasted editor classes/IDs.
  - `keep_comments` default false: comments are removed unless requested in safe-html mode.
- Dangerous blocks are removed with contents where appropriate: script, style, iframe/embed/object, SVG/MathML, forms, media tags, and head-only tags.
- Page copy stays generic and avoids competitor wording/branding/trademarks.

## Explicit out-of-model / deferred items

- Server-side sanitizer guarantees, CSP configuration, and application-specific trust policy are outside this local utility.
- Full HTML5 browser parser parity and complete CSS sanitization are not claimed; the page documents conservative limits.
- URL fetching, bulk batch processing, downloadable files, and CMS integrations are outside this pure text block.
