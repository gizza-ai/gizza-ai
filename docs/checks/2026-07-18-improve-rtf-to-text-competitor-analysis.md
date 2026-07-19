# rtf-to-text — competitor analysis (2026-07-18)

Pre-build competitor scan for the new pure-text tool `rtf-to-text` (strips RTF
control words and groups to produce plain Unicode text). Top reachable real
tools reviewed. **Paraphrased only — no competitor copy, branding, or trademarks
reproduced.**

## Competitors reviewed (top 3 reachable)

### 1. Made in Text — RTF to Text Converter (madeintext.com)
- **Input:** file upload only ("Choose File"); no paste box, no URL.
- **Options:** none — single-step "no extra steps" conversion, no settings.
- **Processing:** strips all RTF formatting, extracts plain text.
- **Output:** plain text shown in a box; **Copy**, **Download .txt**, **Reset** buttons.
- **UX:** minimal, simplicity-first.

### 2. CoolUtils — RTF to TXT (coolutils.com/online)
- **Input:** drag-and-drop / file select; accepts many doc formats, 50 MB cap (online).
- **Options:** choose TXT output; optional custom **header/footer** text; UTF-8 output.
- **Processing:** auto-detects format; strips font/color/bold/italic/underline/spacing,
  **tables and embedded images**. Paragraph breaks → newlines. Tables flattened to text
  with space/tab separators. Output ~30–70% smaller than source.
- **UX:** three-step upload → configure → download; preview panel; server-side (files
  deleted post-conversion, TLS).

### 3. Pixaura — Rich Text to Plain Text (pixaura.com)
- **Input:** paste into a text box (also gives OS-app instructions for files).
- **Options:** produces both plain-text and rich-text versions; pick output type.
- **Output:** shown on page / downloadable.
- **UX:** paste + one action button; step-by-step guidance; no FAQ.

(A 4th/5th — encode64, freetoolmate — were unreachable at scan time: 403 / DNS
failure. Proceeded with the three reachable ones.)

## Table-stakes → our decision

| Capability | In/out of model | Decision |
| --- | --- | --- |
| Strip RTF control words, groups, formatting → plain text | in-model | **Core function** — hand-rolled RTF tokenizer. |
| Preserve paragraph breaks as newlines (`\par`, `\line`) | in-model | **Built** — `\par`/`\sect`/`\page`/`\line` → newline; `\tab` → tab. |
| Unicode: `\uN` escapes (signed, `\ucN` skip count) + `\'hh` hex (cp1252) | in-model | **Built** — correct signed-`\u`, `\uc` fallback-skip, cp1252 hex decode incl. 0x80–0x9F. |
| Skip non-text destinations (fonttbl, colortbl, stylesheet, info, pict, generator, `\*`) | in-model | **Built** — destination set + `\*` ignorable-group handling. |
| Table cells flattened (`\cell`/`\row`) | in-model | **Built** — `\cell` → tab, `\row` → newline. |
| Typographic control words (`\emdash`, `\bullet`, smart quotes, `\~`/`\_`/`\-`) | in-model | **Built** — mapped to the correct Unicode chars. |
| Copy result / Download .txt / Reset | in-model (platform) | **Built** — generator supplies Copy/Reset automatically; `format="text"` gives Download. |
| Flatten to a single spaced line (search / LLM pre-processing) | in-model | **Built** — `line_breaks=collapse` param (our differentiator over the paste-only tools). |
| File upload (.rtf) as page input | out-of-model (pure text tool: paste the markup) | Listed — page takes pasted RTF markup; CLI/chat take `rtf` string. Bulk file batch is a site concern. |
| Custom header/footer injection (CoolUtils) | considered, rejected | Out of scope for a text-extraction tool — adds schema bloat, not extraction. |
| Server-side multi-format (doc/docx/odt) conversion | out-of-model (server) | Out of scope; gizza is browser-local. Sibling blocks handle `.doc`/`.docx`. |
| Alternate codepages (`\ansicpgNNNN` other than 1252) | out-of-model (niche) | Documented limit: `\'hh` decoded as Windows-1252 (the RTF `\ansi` default); `\uN` covers all Unicode regardless. |

## Descriptor shipped

- `rtf` (string, required) — the raw RTF markup.
- `line_breaks` (enum `preserve` | `collapse`, default `preserve`) — keep paragraph
  newlines, or flatten all whitespace to single spaces.
