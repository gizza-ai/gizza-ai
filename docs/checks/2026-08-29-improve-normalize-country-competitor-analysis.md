# normalize-country — competitor analysis (2026-08-29)

Scan run while finishing the tool, per the create-next-tool / improve-tool procedure. All competitor behaviour below is paraphrased from observation and search snippets; no competitor copy, branding, markup or trademarks were reproduced, and out-of-model items are listed rather than built.

## Scope

Searches used: "online country name normalizer country code converter alpha-2 alpha-3 CSV batch" and "country name to ISO 3166 code converter normalize country names alpha-2 alpha-3 numeric batch". The scan focused on reachable paste-and-go country-code converters and lookup pages that expose their feature shape clearly.

| # | Tool (function) | Reachable | Shape |
|---|-----------------|-----------|-------|
| 1 | Trace My IP country ISO code converter | yes | Batch converter between country names, alpha-2, alpha-3 and numeric codes |
| 2 | CountryConverter.com | yes | Dedicated country-name/code converter with list input |
| 3 | JLV DevTools country-code lookup | yes | Browser lookup with ISO codes plus extra reference fields |
| 4 | country-codes.mysite.wiki finder/exporter | yes | Search/batch refine/export workflow |
| 5 | TextQuery bulk ISO codes to country names | yes | Bulk code-to-name conversion for pasted data |

Static reference lists such as Wikipedia, IBAN country codes and Nations Online were used as baseline expectations for ISO alpha-2, alpha-3 and numeric coverage, not counted as direct competitors because they are primarily tables rather than normalization tools.

## What table-stakes look like

**1 — Trace My IP converter.** The search listing advertises conversion from country name to alpha-2, alpha-3 and numeric, plus reverse conversion from alpha-2/alpha-3 back to country name. Batch input and list output are the important UX signals. It appears oriented around the official ISO forms rather than fuzzy cleanup.

**2 — CountryConverter.com.** Similar one-purpose converter: country name or list to ISO alpha-2, alpha-3 or numeric, and reverse conversion from ISO codes to country names. Its market position confirms that users expect bidirectional lookup and batch operation in one page.

**3 — JLV DevTools lookup.** Adds reference context beyond ISO fields — numeric code, telephone dial code, TLD, currency and capital — and positions the tool as instant browser-based lookup. Those extra enrichment fields are useful in a directory, but only the ISO fields fit this pure normalizer.

**4 — country-codes.mysite.wiki finder/exporter.** The listing describes searching every ISO 3166-1 entry by name, alpha-2, alpha-3 or numeric, pasting a batch list, refining matches and exporting. The refinement step implies ambiguity handling and auditability are table-stakes for messy data imports.

**5 — TextQuery bulk converter.** Focuses on bulk ISO code to full country name conversion with pasted rows from CSV/Excel. It supports alpha-2 and alpha-3 and ignores special characters. That reinforces one-column output modes and punctuation-tolerant matching.

## Table-stakes checklist → shipped decisions

Every item is tagged in-model (browser-local, pure Rust/wasm, no account, no server) or out-of-model, and every in-model item appears in the descriptor or page controls.

| Table-stake | Verdict | Where it landed |
|---|---|---|
| Country name → ISO alpha-2 | in-model | `output = "alpha2"` |
| Country name → ISO alpha-3 | in-model | `output = "alpha3"` |
| Country name → ISO numeric | in-model | `output = "numeric"`, zero-padded to three digits |
| ISO alpha-2/alpha-3/numeric → country name | in-model | exact resolver accepts all code forms; `output = "name"` |
| Show all canonical fields at once | in-model | default `output = "table"`, plus `csv`/`json` |
| Batch pasted input | in-model | multiline `input`, up to 1000 items |
| Explicit split controls | in-model | `delimiter = auto|newline|comma|semicolon|pipe|tab` |
| Spreadsheet/script export | in-model | `output = csv|json` |
| Everyday names and common variants | in-model | curated aliases: USA/U.S., UK, Burma, Zaire, Czech Republic, endonyms and demonyms |
| Punctuation/accent tolerant matching | in-model | ASCII/accent folding and punctuation-insensitive lookup |
| Fuzzy typo correction | in-model | `fuzzy` checkbox, default true, with ambiguous ties reported instead of guessed |
| Audit problem rows | in-model | `on_unmatched = only`; also `keep`, `blank`, `omit` for row alignment choices |
| Dedupe repeated countries | in-model | `dedupe` checkbox |
| Sort output | in-model | `sort = input|asc|desc` |
| Flag emoji output | in-model | `output = "flag"` and flag column in table/csv/json |
| Extra enrichment fields such as phone dial code, TLD, currency, capital | out-of-model for this tool | useful reference data, but this backlog item is specifically ISO 3166 normalization; adding secondary datasets would expand scope and maintenance burden |
| Interactive manual match-refinement UI | out-of-model | would require stateful per-row UI. The stateless page instead marks ambiguous/unmatched rows and provides `on_unmatched=only` for audit passes |
| Upload whole CSV/XLSX and rewrite a chosen column | out-of-model | separate CSV/file tooling; this tool handles pasted country columns and emits a converted column/table |
| Hosted enrichment or account-backed dataset | out-of-model | would break browser-local/no-server behaviour |

## UX control patterns adopted

- `[[example]]` preset chips cover messy mixed input, names to alpha-2, codes to everyday names, and unmatched-row auditing.
- `[input.labels]` make enum controls read as user outcomes instead of internal values.
- The main input is a multiline pasted-column field with placeholders for codes, names, comma-inverted names, numeric code and typo examples.
- Boolean controls expose the two high-impact cleanup choices competitors either hide or lack: dedupe and typo correction.
- The page states the 1000-item cap, ambiguous-match behaviour and ISO-only scope so users know why geopolitical or historical entities may remain unresolved.

## Considered, deferred, rejected

- **Dial codes, TLDs, currencies and capitals**: rejected for this tool. They are reference-directory fields, not ISO 3166 normalization outputs, and introduce datasets with different update cadences.
- **Per-row manual correction dropdowns**: deferred as out-of-model for the current generated pure-tool page. The shipped audit mode is deterministic and scriptable.
- **File upload / full CSV rewriting**: deferred to file/CSV-specific tools. Paste-in, one-column conversion is the common denominator across scanned competitors and works in CLI/chat/page surfaces.
- **Resolving unofficial entities such as Kosovo or the European Union**: deliberately not shipped as ISO 3166-1 country results. They remain unmatched so data pipelines do not silently produce non-standard codes.
