# ris-bibtex-converter — competitor analysis (2026-08-29)

Scan run while finishing the tool, per the create-next-tool / improve-tool procedure. All
competitor behaviour below is **paraphrased from observation** — no competitor copy, branding,
markup or trademarks were reproduced, and out-of-model items are listed, not built.

## Scope

Two web searches ("convert RIS to BibTeX online converter tool" and "convert BibTeX to RIS online
converter"), then five reachable competitor tools were skimmed. Both directions were searched
separately because most of the market ships them as two one-way pages rather than one converter.

| # | Tool (function) | Reachable | Shape |
|---|-----------------|-----------|-------|
| 1 | thelatexlab.com RIS→BibTeX **and** BibTeX→RIS (two sibling pages) | yes | Browser-side, per-direction page, a few options |
| 2 | converterpanda.com RIS→BibTeX | yes | Browser-side, file-drop, six checkboxes |
| 3 | bruot.org ris2bib | yes | Paste box + Convert button, zero options (cb2Bib-backed) |
| 4 | scispace.com RIS↔BibTeX "agents" | page 403s to a plain fetch; described from its search listing | Server-side, account-shaped, database-backed |
| 5 | citeme.app BibTeX→RIS | yes | Browser-side paste box, no options, download button |

(`MacDumi/bib2ris` and the `asouqi` BibTeX converter were noted as prior art — a Python script and
a multi-format exporter — but the first is not a web tool and the second's page exposed no
readable feature detail, so neither is counted among the five.)

## What each one actually offers

**1 — thelatexlab.com.** The closest analogue and the only scanned vendor covering both
directions, as two separate pages. Input by file drag-and-drop or paste; processing is stated to
be entirely in the tab with no network call. RIS→BibTeX exposes an output-dialect choice
(BibLaTeX, keeping UTF-8 for biber, versus legacy BibTeX, converting accents to LaTeX accent
macros) and a per-row dropdown to override the inferred entry type on individual records. Its
documented type map is JOUR/CHAP→incollection, CPAPER and CONF→inproceedings,
THES and MTHES→phdthesis/mastersthesis, RPRT→techreport, ELEC and DATA→misc as the fallback;
field notes cover SP/EP joined into a page range with an en dash, DO→doi, SN→issn or isbn, UR→url.
The BibTeX→RIS page adds a target-format pill (RIS plus CSV and several citation styles), decodes
accent macros back to UTF-8, drops brace protection because RIS has no case protection, splits
page ranges back into SP/EP, and reports per-entry errors inline. No limits are stated on either
page. FAQ topics: what RIS is and where to get it, which managers' exports work, troubleshooting
a failed parse (BOM, missing `ER`, misidentified format), whether DOI/ISSN/ISBN/URL survive,
privacy, and an upsell to a paid cleanup service.

**2 — converterpanda.com.** RIS→BibTeX only. File drag-and-drop or picker (`.ris`, and `.txt`
holding RIS); states all processing is local with nothing sent to a server, and warns that files
over ~50 MB may be slow. Six user options, all booleans: include URL, include DOI, include
abstract, include keywords, auto-generate citation keys, sort entries alphabetically. Keys are
first-author surname plus year (`smith2023`), with `a`/`b` suffixes on collision. Type map covers
JOUR, BOOK, CHAP, CONF, THES, RPRT. FAQ topics: RIS versus BibTeX, supported entry types, data
security, converting several files, how keys are made, size limits.

**3 — bruot.org.** A paste box and a Convert button. No options at all, no documented mapping, no
limits, one direction only; output is produced by the cb2Bib application behind the page.

**4 — scispace.com.** Server-side and account-shaped rather than a paste-and-go page. Its listing
advertises type/field mapping, DOI and PMID resolution against a hosted bibliographic database,
optional deduplication across a library, and export to `.bib`, CSV and CSL-JSON. The page would
not serve to a plain fetch, so its option set could not be inspected directly and nothing about
its UI is claimed here.

**5 — citeme.app.** BibTeX→RIS, paste only, explicitly no signup. Parses each entry and maps
fields to RIS tags; claims all the standard entry types; batch input by pasting a whole `.bib`;
copy or download the result. No options, no documented limits.

## Table-stakes checklist → where each one landed

Every item is tagged **in-model** (browser-local, pure Rust/wasm, no account, no server) or
**out-of-model**, and every in-model item is in the shipped descriptor.

| Table-stake (seen at 1–5) | Verdict | Where it landed |
|---|---|---|
| RIS → BibTeX | in-model | `direction = "ris-to-bibtex"` |
| BibTeX → RIS | in-model | `direction = "bibtex-to-ris"` — one tool, not two pages |
| Guess the direction from the input | in-model, **extended** | `direction = "auto"` (default) sniffs `TY  - ` vs `@type{`; no scanned competitor does this because each page is single-direction |
| Paste as input | in-model | multiline `input` field |
| Reference-type map (JOUR, BOOK, CHAP, CONF/CPAPER, THES→phd/masters, RPRT, ELEC, DATA, UNPB, COMP) | in-model | `ris_type_to_bibtex` / `bibtex_type_to_ris`, both ways, with `misc`/`GEN` as a non-dropping fallback |
| `M3` decides phdthesis vs mastersthesis | in-model | same map; the reverse writes `M3  - PhD thesis` / `Master's thesis` |
| SP + EP joined into a `pages` range with `--` | in-model | `join_pages`, incl. single page, open range, and an already-hyphenated SP |
| `pages` split back into SP/EP | in-model | `split_pages`, en/em dash and `--` tolerant |
| DO→doi, UR→url, SN→isbn *or* issn by entry type, AB→abstract, KW→keywords | in-model | full field map both ways (see the page's "what maps to what" section) |
| Auto-generated cite keys, surname+year, `a`/`b` on collision | in-model, **extended** | `key_style` with four styles rather than one on/off checkbox; collision suffixing is unconditional so the output never has duplicate keys |
| Include/exclude abstract | in-model | `include_abstract` (default on) |
| Include/exclude keywords | in-model | `include_keywords` (default on) |
| Sort the output | in-model, **extended** | `sort` = source/key/year/type, versus competitor 2's single "alphabetically" checkbox |
| Decode LaTeX accent macros to UTF-8 on the way to RIS | in-model | `translate_latex` (default on), reusing the sibling `.bib` reader's ~50 accent/ligature rules |
| Brace protection dropped for RIS (`{DNA}`→`DNA`) | in-model | same flag |
| Per-entry error reporting instead of silent garbage | in-model | hard, named errors ("could not tell whether the input is RIS or BibTeX…", "no BibTeX entries found…") that say what was expected |
| Tolerate blank lines, trailing whitespace, missing `ER` | in-model | RIS reader closes an unterminated record at the next `TY`; wrapped continuation lines fold back |
| Stated privacy / local processing | in-model | wasm in the tab; the FAQ says so plainly |
| Include/exclude URL and DOI as separate checkboxes (competitor 2) | **deliberately not shipped** | a DOI or URL is one short line that every citation style may want; two more checkboxes for it is UI tax. `translate_latex` already exempts both from escaping so they stay clickable |
| File upload / drag-and-drop (1, 2) | **out-of-model for a pure tool** | the generated pure-tool page is a paste field; file input is reserved for file-input blocks |
| Per-row entry-type override dropdown (1) | **out-of-model** | needs interactive per-record UI state; the page is a stateless function of its inputs. The `M3` heuristic plus explicit `direction` cover the cases it exists for |
| DOI/PMID resolution, dedup against a hosted database (4) | **out-of-model** | needs a backend and a licensed dataset; nothing is uploaded here |
| Export to CSV / CSL-JSON / APA / MLA / … (1, 4, 5) | **out-of-model for this tool** | separate converters — the sibling `bibtex-to-csv` tool is the CSV path |
| Download the result as a file | in-model, already generic | `format = "text"` pages get a Download link from the shared runtime |
| Sample/preset input (1, 2) | in-model | four `[[example]]` preset chips, which prefill *and* run |

## Gaps we close that none of the five offer

Deliberate, all in-model and all schema-visible:

- **One bidirectional tool with auto-detection.** Every scanned vendor ships one direction per
  page; pasting a `.bib` into a RIS→BibTeX page there gets you an error or an echo. Here the
  default sniffs the input, and forcing `direction` turns a wrong guess into a named parse error.
- **Four cite-key styles**, including `ris-id` (reuse the exporter's own `ID` tag, which Zotero and
  EndNote often write, so keys survive a round trip) and `numeric`. Competitor 2 offers one style
  behind a yes/no checkbox; the rest do not document key generation at all.
- **LaTeX escaping on the way *into* BibTeX.** Every competitor documents decoding on the way out;
  none mentions escaping `& % $ # _ { } ~ ^ \` on the way in, which is the difference between a
  `.bib` that compiles and one that dies on a title containing an ampersand. `url` and `doi` are
  exempted so links stay usable.
- **ASCII-folded cite keys.** `Erdős, Pál` keys as `erdos1959…` rather than emitting non-ASCII
  bytes into a `\cite{}` argument.
- **`indent`** (0–16) for teams whose `.bib` files are diffed or linted.
- **A stated, enforced input cap** (1,000,000 bytes) with a real error, instead of competitor 2's
  informal "over 50 MB might be slow".
- **Month and access date crossing over** (`PY  - 2024/03/09/` → `year`+`month`, `Y2` ↔ `urldate`
  normalised to `YYYY-MM-DD`), which no scanned page documents.

## UX control patterns adopted

- Four `[[example]]` preset chips (RIS→BibTeX article, BibTeX→RIS book, reference-manager export
  with numbered keys and no abstract, multi-record conference input at indent 4 sorted by year) —
  the declarative answer to competitors' sample-data buttons, and they cover both directions.
- `[input.labels]` on all three enums so the dropdowns read as outcomes (`shannon1948mathematical`)
  rather than as internal value names.
- `kind = "slider"` for `indent`, a bounded 0–16 range that is explored rather than typed.
- Limits and lossiness stated on the page, not only in an error: the byte cap, the dropped-field
  list (RIS `AD`/`DB`/`DP`, BibTeX `crossref`/`annote`), and the fact that a RIS→BibTeX→RIS round
  trip regenerates `ID` from the invented key unless `ris-id` was used.

## Considered, deferred, rejected

- **Legacy-BibTeX output dialect** (competitor 1): re-encoding UTF-8 accents *into* LaTeX macros
  (`ä` → `\"a`) for engines that predate biber. In-model and buildable — noted here rather than
  silently dropped — but deferred: every current engine reads UTF-8 `.bib`, cite keys are already
  ASCII-folded so the portability trap that matters is closed, and `translate_latex = false`
  already gives byte-for-byte passthrough. Worth adding as a `dialect` param if asked for.
- **Separate include-URL / include-DOI checkboxes**: rejected as UI tax (see the table).
- **Merging `journal` and `booktitle`**: rejected — an `@inproceedings` record can legitimately
  carry both, and RIS keeps `JO` and `T2` distinct too.
- **Enriching records from an external database**: out of model, and it would break the promise
  that nothing leaves the tab.
