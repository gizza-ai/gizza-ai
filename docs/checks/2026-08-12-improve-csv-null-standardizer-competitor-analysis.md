# csv-null-standardizer — competitor analysis (2026-08-12)

Scan run **before** implementation, per `/create-next-tool` step 4. One web search
("standardize missing value tokens NA N/A null NaN in CSV online tool replace null
representation"), then the top real competitors were skimmed. Everything below is
**paraphrased**; no competitor copy, branding, or trademark is reproduced, and no
competitor wording was carried into our page.

## Competitors skimmed

| # | Competitor | What it is | Reachable |
|---|---|---|---|
| 1 | CSVTools "fill missing values" (csvtools.com) | Browser-local CSV utility that replaces empty cells with a chosen value | yes |
| 2 | pandas `read_csv` / `to_csv` (`na_values`, `keep_default_na`, `na_rep`) | The de-facto reference implementation for missing-token vocabularies and output representation | yes (API reference; the narrative "Working with missing data" guide has no CSV-specific params, so the API page was used instead) |
| 3 | CSVNormalize (csvnormalize.com) | Marketed "intelligent" CSV cleanup / missing-value platform | yes (marketing article; no parameter documentation published) |

A fourth candidate class (spreadsheet find-and-replace, e.g. MATLAB `standardizeMissing`)
was skimmed only as corroboration for the token-list idea and is not profiled separately.

## Table-stakes observed, with fit-to-model tags

| # | Capability observed | Where seen | Tag | Decision |
|---|---|---|---|---|
| 1 | Configurable list of strings that count as missing (`na_values`) | pandas, CSVNormalize (implicit) | **in-model** | Built: `na_tokens`, comma-separated, editable in place |
| 2 | A sensible built-in default vocabulary of NA tokens | pandas default NA list | **in-model** | Built: default `NA,N/A,N.A.,#N/A,#N/A N/A,#NA,NULL,NIL,NaN,None,<NA>,-,--,?` |
| 3 | Choose the single output representation for missing (`na_rep`, "fill value") | pandas `to_csv`, CSVTools | **in-model** | Built: `replace_with` (blank by default) |
| 4 | Blank / whitespace-only cells treated as missing | CSVTools (its whole definition), pandas | **in-model** | Built: `blank_is_missing` (on by default) |
| 5 | Delimiter configuration, incl. auto-detect | CSVTools ("auto-detect or manual") | **in-model** | Built: `delimiter` accepts `auto`, a single char, or `comma`/`tab`/`semicolon`/`pipe` |
| 6 | Header-row toggle so the header is preserved untouched | CSVTools | **in-model** | Built: `header` (on by default) |
| 7 | Case-insensitive matching of tokens | pandas ships both `NULL` and `null`, `NaN` and `nan` in its defaults | **in-model** | Built: `case_sensitive` (off by default, so one entry covers every casing) |
| 8 | Trim surrounding whitespace before comparing | pandas `skipinitialspace`, CSVTools' whitespace-only rule | **in-model** | Built: `trim` (on by default; only affects matching, non-missing cells stay verbatim) |
| 9 | Per-column scoping. CSVTools states this as an explicit **limitation** — it fills every column uniformly and tells users to re-run the tool on extracted columns | CSVTools | **in-model** | Built: `columns` accepts column names (with a header) or 1-based indices; blank = all columns. This closes their stated gap |
| 10 | Output quoting control (they expose a configurable quote character) | CSVTools | **in-model** (reshaped) | Built as `quote_style` = `minimal`/`always`/`never`. A configurable quote *character* was rejected (below); the real user need behind it — emitting `""` so a DB loader can tell an empty string from NULL — is served by `always` |
| 11 | Paste / drop-file / fetch-URL input methods | CSVTools | **out-of-model** for file+URL | Listed, not built. Pure tools on this platform take pasted text; a URL fetch would break the local-only promise |
| 12 | Statistical imputation (mean / median / mode / regression / KNN) | CSVNormalize, pandas `fillna` | **out-of-model** for this tool | Listed, not built. Standardizing a token is not imputing a value, and this repo already ships a dedicated imputer block |
| 13 | ML / "semantic context" detection of missingness | CSVNormalize | **out-of-model** | Listed, not built. Needs a model; this toolkit is pure-Rust + ffmpeg |
| 14 | Saved templates / reusable workflows, accounts | CSVNormalize | **out-of-model** | Listed, not built. No accounts, no server state |
| 15 | Downloadable result file | CSVTools | already covered | The generator gives every `format = "text"` page a Download link and a Copy button for free |

## In-model gaps considered and rejected (with reasons)

- **`keep_default_na`-style "append to the defaults" flag** (pandas). Rejected as redundant
  schema bloat: our `na_tokens` field is pre-filled with the full default list and edited in
  place, so adding a token means typing it into a list you can already see. A separate
  merge/replace flag would only be needed if the defaults were invisible.
- **Configurable quote *character*** (CSVTools). Rejected: RFC 4180 double-quote input is what
  every real-world CSV uses here, and a wrong quote char silently mangles data rather than
  erroring. The genuine need behind the option is met by `quote_style` (item 10).
- **Output delimiter / line-ending conversion.** Rejected: dedicated blocks already own that
  (`csv-change-delimiter`, `csv-cleaner`), and duplicating them would blur this tool's job.
- **A replacement-count summary appended to the output.** Rejected: the output must stay
  paste-able, byte-clean CSV. Counting missing cells is the job of the existing
  `missing-value-report` block, which the page links to in prose.

## Design decisions carried into the descriptor

Nine parameters, one of them a fixed-choice enum:

`input` (required text) · `delimiter` (`auto` / char / name, default `,`) ·
`na_tokens` (default list above) · `replace_with` (default blank) ·
`blank_is_missing` (default true) · `case_sensitive` (default false) ·
`trim` (default true) · `header` (default true) · `columns` (default all) ·
`quote_style` (`minimal` | `always` | `never`, default `minimal`).

Page: preset chips for the four representations people actually target (blank cell, `NULL`,
`NA`, and the Postgres `COPY` sentinel `\N`), a worked input→output example, three FAQ
accordions, and an explicit limits section covering ragged rows, `never`-quoting risk, and
the fact that this tool normalizes tokens rather than imputing values.

## Not verifiable here

The live chat UI lives in the private site repo that consumes this one at a pin, so chat was
not exercised. What was verified locally: the descriptor/schema (incl. the drift guard) that
chat consumes, the CLI, and the generated page.
