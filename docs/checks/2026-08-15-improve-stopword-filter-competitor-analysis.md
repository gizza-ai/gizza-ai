# stopword-filter — competitor analysis (2026-08-15)

Scan run **before** implementing, per `create-next-tool` step 4. One web search
("online stopword remover tool remove stop words from text"), then the top real
competitor pages were skimmed. Everything below is a paraphrased feature
inventory — no competitor copy, wording, or branding is reused anywhere in the
tool.

## Competitors skimmed

| # | Tool | Reachable | Notes |
|---|------|-----------|-------|
| 1 | TextToolz — "Remove Stop Words" | yes | Richest feature set of the three: language select, case toggle, custom list, punctuation preserved by default, copy + download |
| 2 | No Tools Left Behind — "Stop Word Remover" | yes | Single-action English-only tool; long FAQ, no options at all |
| 3 | tools.shounakgupte.com — "Remove Stopwords" | yes | English default list + optional custom word list, reset button, no stats |
| — | textminitools.com — "Remove Stopwords" | **no** (domain parked → Hostinger landing page) | Replaced by #3, per the "replace unreachable competitors" rule |

## Table-stakes inventory

| Capability | Seen on | In model? | Where it landed |
|---|---|---|---|
| Paste text → cleaned text out | 1, 2, 3 | in | core `filter()`, page `format = "text"` |
| Built-in stop-word list per language | 1 (English/Spanish +), 2 (English), 3 (English) | in | `language` enum — english, spanish, french, german, italian, portuguese, dutch, russian, none (8 built-in lists, superset of every competitor) |
| Custom / domain-specific extra stop words | 1, 3 | in | `custom_words` (comma, space, semicolon or newline separated) |
| Case-insensitive matching by default, optional case-sensitive | 1 | in | `case_sensitive` (default false) |
| Keep punctuation and sentence boundaries by default | 1 | in | default behaviour; `remove_punctuation` opts out |
| Strip punctuation too (token-stream output for NLP) | implied by 1's NLP framing | in | `remove_punctuation = true` |
| Protect words that must never be dropped | none — **our differentiator** | in | `keep_words` |
| Show which words were removed / how many | none (1 shows before-after examples only) | in | `output = removed` and `output = stats`, plus the structured chat/CLI fields |
| Copy result button | 1, 3 | in | generator gives every text tool Copy + Reset |
| Download cleaned text | 1 | in | generator gives every `format = "text"` page a Download link |
| Preset examples / one-click demo | 1 (before/after examples in copy) | in | four `[[example]]` chips (SEO keywords, NLP tokens, Spanish, keep-list) |
| Upload a `.txt` file as input | 1, 3 | **out** | The page's pure-tool form is field-only (file inputs are for ffmpeg/media blocks); paste or the CLI covers it |
| Side-by-side original vs cleaned diff view | 1 | **out** | Would need bespoke `custom.js` two-pane UI; `output = removed` covers the "what changed" need declaratively |
| Stemming / lemmatisation alongside removal | 3 (mentioned as related) | **out** | Different tool; a stemmer is its own backlog entry, not a stop-word filter |
| Mobile support | 2 | in (free) | Generated pages are already responsive |

Nothing from the scan was dropped silently: every row is either implemented or
listed as out-of-model above.

## Design decisions taken from the scan

- **Multilingual by default.** Only one competitor offered more than English,
  and only two languages. Eight embedded lists (plus `none` for
  custom-list-only filtering) is the clearest differentiator and matches the
  backlog description ("built-in multilingual stoplists or a custom list").
- **Whole-word, tokenizer-based matching**, not substring replace: `the` never
  eats the `the` inside `theatre`. Contractions stay one token (`don't`).
- **Whitespace repair after removal** — the space left behind by a removed word
  is swallowed and a space stranded before `,` `.` `!` is dropped, so the output
  reads like prose instead of `cat , sat .` (two competitors leave the gaps).
- **Three output views** (`text`, `removed`, `stats`) instead of one, because
  reviewers of an SEO/NLP pipeline usually want to audit what got dropped.
- **Stated cap** (200,000 characters) on the page — no competitor states one,
  but an unstated limit is worse than a stated one.

## Not copied

No competitor copy, FAQ wording, branding, or trademark appears in
`page/meta.toml` or `page/content.md`; the stop-word lists are the standard
public-domain closed-class vocabularies of each language (articles,
prepositions, pronouns, auxiliaries), written out here directly.
