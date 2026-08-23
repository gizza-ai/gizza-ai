# bibtex-to-csv — competitor analysis (2026-08-23)

Scan run BEFORE implementing, per the create-next-tool procedure. All competitor behaviour below is
**paraphrased from observation** — no competitor copy, branding, or trademarks were reproduced, and
out-of-model items are listed, not built.

## Scope

One web search for "convert BibTeX .bib file to CSV online tool". Three reachable, real competitor
tools were skimmed:

| # | Tool (function) | Reachable | Shape |
|---|-----------------|-----------|-------|
| 1 | thelatexlab.com BibTeX→CSV converter | yes | Browser-side, single fixed conversion |
| 2 | bibtex.com BibTeX→CSV converter (Paperpile-backed) | yes | Server-side job, upload/paste |
| 3 | convert.guru BibTeX converter | yes | Generic multi-format document converter |

(GitHub scripts `jcwkroeze/bibtex-csv` and `oezguensi/BibTex-to-CSV` were noted as CLI prior art —
both emit one row per entry with the union of fields — but they are not web tools, so they are not
counted among the three.)

## What each one actually offers

**1 — thelatexlab.com.** The most directly comparable. Zero user-configurable options: CSV only, a
fixed 14-column schema in a fixed order (entry type, citation key, title, authors, year, a merged
journal/booktitle column, volume, issue, pages, DOI, ISBN, ISSN, publisher, URL), mandatory UTF-8
BOM for Excel, comma delimiter only, RFC-4180 quoting applied automatically. Behavioural details it
advertises: LaTeX accent macros are decoded to real UTF-8 characters, brace-protected acronyms lose
their protective braces, multiple authors are joined with the BibTeX ` and ` separator, absent
fields stay present-but-empty rather than being dropped, and unparseable entries are reported
inline. Input by file picker, drag-and-drop, or paste. FAQ topics: why the BOM, importing into
citation managers, accent handling, troubleshooting parse failures, browser-only processing.

**2 — bibtex.com.** Upload / drag-and-drop / paste plus a sample-data button, a format selector, a
convert button and a download link. No conversion options are exposed at all, and the output column
set is not documented on the page. Processing is server-side (it states input and output are deleted
24 h after the job), backed by a hosted bibliographic database that the input is matched against.
FAQ topics: how the conversion works, what a .bib file is, what a CSV file is.

**3 — convert.guru.** A generic converter: BibTeX in, many formats out (CSV, RIS, XML, RTF, DOCX,
ODT, TEX, HTML, …). Upload or drag-and-drop, a preview, then pick an output format. No field-level
or delimiter options; no documented limits.

## Table-stakes checklist → where each one landed

Every item is tagged **in-model** (browser-local, pure Rust/wasm, no account/server) or
**out-of-model**, and every in-model item is in the shipped descriptor.

| Table-stake (seen at 1–3) | Verdict | Where it landed |
|---|---|---|
| One CSV row per entry, standard bibliographic column set | in-model | `columns = "standard"` (default): `type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url` — `journal` and `booktitle` stay separate rather than merged, which is truer to BibTeX |
| Entry type + citation key as columns | in-model | first two columns of every mode |
| Missing fields stay empty, not dropped | in-model | core emits `""` for absent fields |
| RFC-4180 quoting / escaping | in-model | `csv_escape` in core; quotes only when needed |
| LaTeX accent macros → UTF-8 (`\"a`→ä, `\'e`→é, `\c{c}`→ç, `\ss`→ß, …) | in-model | `decode_latex` (default on), ~50 accent/ligature/symbol rules + dash and quote ligatures |
| Brace-protected acronyms lose braces (`{DNA}`→`DNA`) | in-model | same flag |
| Multi-author join with the BibTeX ` and ` separator | in-model | `author_separator = "and"` (default), plus `semicolon`/`comma`/`pipe` |
| UTF-8 BOM for Excel | in-model | `bom` checkbox (default **off** — clean output by default, one click for Excel) |
| Paste as input | in-model | multiline `bibtex` field |
| Inline reporting of unparseable entries | in-model | strict parse errors that name the byte offset/entry (`unterminated @article entry 'smith2020'…`) |
| Header row | in-model | `header` (default true) — implied by every competitor's output, exposed here |
| Fixed comma delimiter | in-model, **extended** | `delimiter` enum comma/semicolon/tab/pipe — the semicolon case is what makes the output openable in comma-decimal-locale Excel |
| File upload / drag-and-drop | **out-of-model for a pure tool** | the generated pure-tool page is a paste field; media/file upload is reserved for file-input blocks |
| Server-side job + 24 h retention (competitor 2) | out-of-model | nothing is uploaded here, so there is nothing to retain |
| Matching input against a hosted bibliographic database to enrich entries (competitor 2) | out-of-model | needs a backend + dataset |
| Other output formats: RIS, XML, DOCX, ODT, HTML (competitor 3) | out-of-model **for this tool** | separate converters, not this tool's job |
| Sample-data button (competitor 2) | in-model | shipped as `[[example]]` preset chips, which prefill *and* run |

## Gaps we close that none of the three offer

Deliberate, all in-model and all schema-visible:

- `columns = "all"` — union of every field present across the entries, alphabetised after
  `type,key`. The GitHub CLI prior art does this; no web tool scanned does.
- `columns = "custom"` + `custom_columns` — pick and order exactly the columns you want (tag-list
  control on the page).
- `author_format` — `bibtex` (verbatim), `last-first` (`Curie, Marie`), `first-last`
  (`Marie Curie`). Competitor 1 only ever passes the source spelling through.
- `expand_strings` — resolve `@string` macros and `#` concatenation before emitting (a real BibTeX
  feature; leaving it off exposes the raw macro name).
- `sort` — `source` / `key` / `year` / `type`.

## UX control patterns adopted

- Preset chips (`[[example]]`) instead of a single sample-data button: three one-click presets
  covering a plain article, all-fields mode, and the Excel/semicolon+BOM path.
- Friendly `<select>` labels via `[input.labels]` so the delimiter dropdown shows the actual
  character.
- `kind = "tag-list"` for `custom_columns` (column names never contain commas, so pills are safe
  here).
- Stated limits on the page rather than only in an error: 1,000,000-byte input cap, 200-column cap
  in custom mode.

## Considered, rejected

- Merging `journal`/`booktitle` into one column like competitor 1 — lossy for `@inproceedings`
  entries that carry both. `columns = "custom"` can reproduce their layout if wanted.
- A mandatory BOM. It corrupts naive `read_csv` consumers that don't strip it; a default-off
  checkbox with an Excel-labelled preset chip gets the same outcome without the tax.
