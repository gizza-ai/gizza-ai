# html-email-to-text — competitor analysis (2026-07-18)

Scan run before implementing the tool. All notes are paraphrased; no competitor copy,
branding, or trademarks were copied. Out-of-model items are listed, not built.

## Competitors scanned (top real tools for "HTML email → plain text")

1. **Mailmeteor — HTML to Text** — paste HTML (page or email) → plain text on a button click.
   General-purpose stripper positioned for emails.
2. **CodeItBro — HTML to Plain Text** — paste HTML → instant plain text; whitespace controls
   (preserve line breaks vs collapse excess) and **link handling toggle: keep URL alongside
   anchor text, discard, or as footnotes**.
3. **unboxd — Plain Text Email Converter** — email-focused; preserves links/lists/tables and
   renders hyperlinks as **text followed by URL in parentheses** (`Click here (https://…)`).
4. **Mailchimp — HTML to Text** (email design reference) — auto-generates the text alternative
   of an HTML campaign; hard-wraps to a plain-text-email width.
5. **Sercante — HTML to Email Text Converter** — strips extra spacing/line breaks for a clean
   plain-text email body.

(Textfixer and AIFreeForever were also seen but are bare tag-strippers with no email options;
used only to confirm the table-stakes baseline.)

## Table-stakes (each tagged in-model / out-of-model)

| Capability | Competitor(s) | Decision |
|---|---|---|
| Strip tags → readable text, keep paragraphs/lists | all | **in-model** — `nanohtml2text` base conversion |
| Decode HTML entities (`&amp;`, `&nbsp;`, …) | Mailmeteor, AIFreeForever | **in-model** — done by base conversion + our footnote-URL decoder |
| Link handling: text-only / inline URL / footnotes | CodeItBro, unboxd | **in-model** — `links` enum (`text`/`inline`/`footnote`); this is the core differentiator |
| Inline URL as `text (url)` | unboxd (default) | **in-model** — our default `inline` mode |
| Footnote/reference list of URLs | CodeItBro | **in-model** — `footnote` mode appends `[n] url` block |
| Collapse excess blank lines / whitespace | CodeItBro, Sercante | **in-model** — base normalize (3+ newlines → 2, trim) |
| Hard-wrap to plain-text-email width (~72) | Mailchimp | **in-model** — `wrap` integer/slider (0–200, 0 = off), never splits URLs |
| Preset examples / one-click try | (UX pattern) | **in-model** — 3 `[[example]]` preset chips |
| Friendly `<select>` for link mode | (UX pattern) | **in-model** — `[input.labels]` friendly names |
| `mailto:`/`tel:` cleanup, drop `#`/`javascript:` | (quality detail) | **in-model** — implemented in `emit_link` |

## Out-of-model / considered, not built

- **Preserve HTML tables as ASCII grids** — competitors (unboxd) claim table "preservation",
  but faithful column alignment needs a full table layout engine; we flatten tables to their
  text content (stated on the page). Considered, not built.
- **Image `alt` text emission / tracking-pixel reporting** — `nanohtml2text` drops images and
  does not surface `alt`; emitting it would need our own parser pass. Listed as a known limit,
  not built (candidate for a later pass).
- **`format=flowed` (RFC 3676) soft-wrap output** — niche mail-transport encoding; the common
  need is hard-wrap, which we ship. Considered, rejected (schema bloat for little user value).
- **Uppercase/underlined heading styling** — some text-email generators upcase `<h1>`; this is
  stylistic and not consistently expected. Considered, rejected.
- **Server/cloud batch, accounts, API keys** — out of gizza's browser-local, no-account model.

## Result

Every table-stake landed in the descriptor (`html`, `links` enum, `wrap`) or is explicitly
listed above as out-of-model. The tool is materially distinct from the existing generic
`html-to-text` block (which is a single-param stripper that drops URLs): this one is
email-focused with three link-rendering modes and configurable plain-text-email wrapping.
