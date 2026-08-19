# markdown-flashcards — competitor analysis (2026-08-14)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is
**paraphrased**; no competitor copy, branding or trademarks were reused. The reference for
the export format is Anki's own public import documentation, not a competitor's wording.

## Sources skimmed

| # | Tool | Shape | Reachable |
| --- | --- | --- | --- |
| 1 | IIENSTITU "Anki deck generator" (browser tool) | paste notes → TSV/CSV download | yes |
| 2 | MD2Anki (hosted SaaS) | upload Markdown → `.apkg` | yes |
| 3 | markdown-to-anki-csv-converter (open-source Python CLI, J. Hagspiel) | Markdown files → CSV | yes |
| — | Anki manual, "Importing text files" | the target file format itself | yes (spec, not a competitor) |

## What they ship

**1. IIENSTITU Anki deck generator.** Auto-detects four note shapes: a delimited
`question|answer` line (accepts `|`, `-`, tab, `;`, `=>`), bold-to-cloze conversion with
sequential `c1/c2/c3` numbering, `word: definition` vocabulary lines, and Markdown headings
(heading = question, following text = answer). Exports TSV (aimed at Anki) plus CSV and a
plain-text legacy variant, and advertises compatibility with Quizlet/RemNote/Mochi/Memrise.
Parsing is deterministic and client-side; a live preview renders the parsed cards as you
type so mistakes are fixed in the source text before export. No deck/tag/separator controls
beyond the auto-detection, and no stated card limit.

**2. MD2Anki.** Hosted, account-based, freemium (a monthly document quota on the free tier,
paid tiers for unlimited/bulk/history). Handles headings, code blocks and tags, and exports
the native `.apkg` package rather than a text file. UX: a rendered/source Markdown preview
toggle and a downloadable sample deck. Detailed syntax rules, cloze support and deck-naming
behaviour are behind the sign-up wall.

**3. markdown-to-anki-csv-converter.** A local script over already-structured Q/A Markdown,
emitting CSV for the Basic note type. Notable for two operational details: it copies
referenced images into Anki's `collection.media` folder and rewrites the reference, and it
documents that Markdown code formatting does not survive into Anki (so code must be
converted, not passed through). It expects the importer to tick "Allow HTML in fields",
rename the target deck manually, and has no flags/config file.

**Anki import spec (target format).** Fields separated by comma, semicolon, tab, space, pipe
or colon; RFC-4180 quoting with `""` for a literal quote; `#key:value` header directives at
the top of the file — `separator`, `html`, `notetype`, `deck`, `tags`, `columns`,
`tags column`, `deck column`, `notetype column`, `guid column`; `#` lines are otherwise
comments; media referenced as `<img src=…>` / `[sound:…]` with HTML allowed.

## Table stakes → decisions

| Capability | Seen in | Verdict | Where it landed |
| --- | --- | --- | --- |
| Multiple input shapes auto-detected | 1 | in-model | `mode=auto` over heading / qa / table / separator (+ explicit `cloze`) |
| Delimited `q<sep>a` lines, several delimiters | 1, 3 | in-model | `mode=separator` with `separator=auto` scanning `::`, `=>`, tab, `\|`, `;`, ` - `, `:`, plus names and literals |
| Bold → sequential cloze (`c1`, `c2`, …) | 1 | in-model | `mode=cloze`, also accepts `==highlight==`; auto-upgrades the note type to `Cloze` |
| Vocabulary `word: definition` lines | 1 | in-model | same separator mode (bullets/numbering stripped first) |
| Heading = question, body = answer | 1, 2 | in-model | `mode=heading` + `heading_level` (0 = auto-pick the most productive level) |
| TSV for Anki, CSV for other apps | 1 | in-model | `field_separator=tab\|comma\|semicolon\|pipe` with RFC-4180 quoting |
| Markdown/code must be converted, not passed through | 3 | in-model | `field_format=html` (default) renders bold/italic/code/links/lists/fenced code and `<br>`; `markdown`/`plain` alternatives |
| Live preview of parsed cards | 1, 2 | in-model | `output=preview` (numbered cards + detected mode + count) and `output=json`; the page recomputes on every keystroke |
| Deck name, tags, note type | 2, 3 (manual there) | in-model | `deck`, `tags`, `notetype` → `#deck:` / `#tags:` / `#notetype:` headers; `tags_from_headings` for per-card hierarchical tags |
| Header directives so the import dialog is pre-configured | Anki spec | in-model | `include_headers` (default on) writes `#separator/#html/#notetype/#deck/#tags/#columns/#tags column` |
| Tags column per card | Anki spec | in-model | table mode's 3rd column; `#tags column:3` emitted when present |
| Duplicate handling | — | in-model (addition) | `dedupe` (default on) drops repeat questions case-insensitively |
| Stated limits | none of them | in-model (addition) | 1,000,000 chars / 5,000 cards, with actionable errors, stated on the page |
| `.apkg` export | 2 | **out-of-model** | a `.apkg` is a zipped SQLite collection; no wasm-safe pure-Rust path. Documented in the FAQ with the text-import alternative |
| Bundling images into `collection.media` | 3 | **out-of-model** | needs local filesystem access to Anki's profile; we reference the image and say so on the page |
| Accounts, quotas, conversion history, bulk upload | 2 | **out-of-model** | needs a backend; gizza tools are local and account-free |
| AI-generated cards from prose | 2 (adjacent) | **out-of-model** | needs a model; the parser stays deterministic |
| Preset chips for common note shapes | 1 (implicit via samples) | in-model | five `[[example]]` chips: heading notes, Q/A, vocabulary, cloze, table+tags |

**Considered, rejected:** a `guid column` / stable-GUID option (Anki supports it for update-on-
reimport, but a deterministic GUID would have to be derived from the question text, which
silently breaks re-import as soon as a typo is fixed — worse than no GUID). Also rejected:
splitting the four "detect" shapes into separate tools; auto-detection with one explicit
override is the smaller surface.

## UX patterns adopted

- Deterministic, client-side parsing with an at-a-glance **preview** output (competitor 1's
  strongest idea) instead of a silent export.
- The detected format is reported back to the user (preview/JSON both name the mode), so
  "why did it parse that way" is answerable without guesswork.
- Preset chips instead of a sample-deck download.
- Limits, HTML-import caveat and the image-media caveat stated on the page rather than
  discovered through a failed Anki import (competitor 3 documents these only in a README).
