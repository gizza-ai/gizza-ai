## About this tool

**Zero-width character cleaner** detects and strips *invisible* characters from
text — the ones that take up no visible width (or masquerade as an ordinary
space) yet still travel along when you copy and paste. They sneak in from web
pages, PDFs, Word documents, chat apps, spreadsheets and "watermarked" AI output,
and then quietly break search, diffing, `==` comparisons, code, CSV imports,
usernames and password fields.

Paste your text and the cleaner removes every invisible character it recognises,
leaving the visible content untouched. Everything runs locally.

### What it removes

- **Zero-width characters** (default on): zero-width space (U+200B), zero-width
  non-joiner (U+200C), zero-width joiner (U+200D), word joiner (U+2060), the
  invisible math operators (U+2061–U+2064), the Mongolian vowel separator
  (U+180E), and the **byte-order mark / ZWNBSP** (U+FEFF) — the classic invisible
  first character of a file.
- **Bidirectional controls** (default on): the invisible left-to-right and
  right-to-left marks, embeddings, overrides and isolates (U+061C, U+200E/U+200F,
  U+202A–U+202E, U+2066–U+2069). These can silently reorder how text is displayed
  and are the basis of the "Trojan Source" code-spoofing trick.
- **Soft hyphens** (default on): U+00AD, an invisible optional line-break hint
  that appears as a stray character when copied out of formatted documents.
- **Non-breaking & unusual spaces** (optional): turn on *Replace non-breaking
  spaces* to convert non-breaking spaces and other unusual Unicode spaces
  (U+00A0, U+2000–U+200A, U+202F, U+205F, U+3000, U+1680) into a normal ASCII
  space. The word gap is kept — only the odd space character is normalised.

Each removed character is deleted by default. Use the **Replacement** field to
substitute something visible (for example `?` or `[zwsp]`) so you can *see* where
the invisible characters were.

### Note on emoji

Modern emoji sequences (like 👨‍👩‍👧 or a flag) use the zero-width joiner
(U+200D). Removing zero-width characters will split those into separate glyphs —
turn *Remove zero-width characters* off if you need to preserve emoji sequences.

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat.

### Common uses

- Clean text pasted from a website, PDF, or Word document before using it.
- Strip a leading byte-order mark (BOM) that breaks CSV, JSON, or code parsing.
- Remove invisible zero-width "watermark" characters some tools inject into text.
- Sanitize usernames, passwords, or search terms that silently fail to match.
- Detect Trojan-Source-style bidi overrides hiding in source code.
