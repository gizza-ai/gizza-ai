# xml-formatter — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/xml-formatter` — pretty-print or minify XML with indentation
control and a well-formedness check. Pure-Rust (`quick-xml`). Pure-text input →
text output: chat + CLI + a page.

## What competitors do

- **Online XML formatters/beautifiers** (codebeautify, freeformatter, etc.) —
  paste XML, get it formatted/validated. Useful, but **the XML is sent to a
  third-party page** (often config/data you'd rather not paste) and they're
  ad-heavy.
- **`xmllint --format`** — local, fast, the reference — but a libxml2 install and
  CLI flags, and not browser/chat runnable.
- **Editor plugins** (VS Code XML, IntelliJ) — great in-editor, but not a one-call
  step for arbitrary pasted XML or automation.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`quick-xml`) compiled to wasm:
   the chat Service Worker, the CLI, and the in-browser page format on-device. The
   XML never leaves the device.
2. **Both directions, one tool.** `pretty` re-indents with a configurable number
   of spaces per level; `minify` strips insignificant whitespace to a single line.
3. **Well-formedness checking with position.** Malformed XML (mismatched tags,
   syntax errors) returns a clear message **with the byte position**, instead of
   silently emitting broken output — so it doubles as a quick validator.
4. **Attribute- and structure-preserving.** Element attributes, self-closing tags,
   comments, CDATA, and processing instructions are passed through faithfully.
5. **Same everywhere.** Identical via chat, CLI, and a `?xml=…&mode=…&indent=…`
   page.

## Honest scope

- **Structural formatting**, not transformation: it re-indents/minifies and checks
  well-formedness; it does not validate against a DTD/XSD schema or canonicalise
  (C14N).
- Significant whitespace inside mixed-content elements is trimmed in the same way
  most formatters do (text is trimmed); this is standard for pretty-printers.

## Tests

5 core unit tests: pretty-prints nested elements with the expected indentation;
honours a custom indent width; minifies an indented document back to one line;
preserves attributes and self-closing tags; and rejects malformed XML (mismatched
tags, junk, empty). Plus the block drift-guard schema test. **CLI verified**
end-to-end (pretty + minify round trip). **Page** verified with Playwright.
`wafer build` instantiates the chat block.
