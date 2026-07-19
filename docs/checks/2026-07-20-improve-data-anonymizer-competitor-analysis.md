# data-anonymizer — competitor analysis (2026-07-20)

New-tool competitor scan done BEFORE implementing (create-next-tool step: scan → design the
descriptor around the in-model table stakes). Paraphrased findings only; no competitor copy.

## Competitors reviewed

1. **ARX Data Anonymization Tool** (arx.deidentifier.org) — open-source desktop/Java framework.
   Privacy models: k-anonymity, l-diversity, t-closeness, k-map, differential privacy.
   Transforms: hierarchy-based generalization, record/attribute/cell suppression,
   microaggregation, top/bottom coding, categorization. User configures k, quasi-identifiers,
   sensitive attributes, generalization hierarchies, suppression limit. Reports residual
   re-identification risk, data-quality/utility metrics, class uniqueness stats.
2. **Amnesia** (amnesia.openaire.eu) — web-based k-anonymity tool. User sets k, picks
   quasi-identifiers, attaches per-QI generalization hierarchies (auto-generatable, e.g.
   zip → city), optional character masking for strings, optional suppression of violating
   records. Reports value distributions, data-quality stats, a solution graph of safe/unsafe
   generalization combinations, and % of records affected.
3. **ANJANA / pyCANON** (github.com/IFCA-Advanced-Computing/anjana) — open-source Python
   library (replaced an unreachable Nature-article page for the same project). Techniques:
   k-anonymity plus (α,k), l-diversity variants, t-closeness, β-likeness, δ-disclosure.
   User supplies: identifier columns (suppressed entirely), quasi-identifier columns
   (generalized), one sensitive attribute, k/l/t levels, a suppression limit (% of records),
   and a hierarchies dict (e.g. age → 5-year / 10-year intervals; strings → "*").

## Table stakes → decision

| Capability (competitor norm) | Tag | Decision in descriptor |
|---|---|---|
| Select quasi-identifier columns | in-model | `quasi` (names or 1-based indices, comma-separated) — required |
| Direct-identifier columns fully suppressed (ANJANA "identifiers", ARX attribute suppression) | in-model | `identifiers` + `label` (default `[REDACTED]`) |
| k parameter | in-model | `k` integer, default 2, 2–100, slider |
| Numeric generalization to intervals (age → 10-year bands) | in-model | auto-detected numeric QI columns binned by `numeric_bin` (default 10); labels `30-39` / `[2.5,5)` |
| String generalization (zip → prefix + `*`, ANJANA `*` suppression) | in-model | text QI columns keep first `text_keep` chars (default 3), rest `*`; `text_keep=0` → whole value `*` |
| Date generalization (date → year) | in-model | ISO-date QI columns → year when `dates_to_year` (default true) |
| Per-attribute generalization level (ARX/Amnesia/ANJANA hierarchies) | in-model (parametric form) | per-column override suffix in `quasi`: `zipcode:100` = bin width / keep-chars for that column |
| Record suppression of under-k classes + suppression reporting | in-model | `suppress` boolean (default false); report shows count + % suppressed |
| Report achieved k (min equivalence-class size), class count/sizes, % at risk | in-model | deterministic k-anonymity report; `output` enum both/csv/report |
| Distinct l-diversity on a sensitive attribute | in-model (distinct variant only) | optional `sensitive` column → `l = min distinct sensitive values per class` |
| Delimiter/header handling for CSV (all tools ingest delimited text) | in-model | `header` (default true), `delimiter` (default `,`) |
| Custom multi-level hierarchy files (VGH), hierarchy editors/wizards | out-of-model | parametric generalization only (bin width / keep-chars / year); no hierarchy files |
| Optimal generalization search (ARX Flash lattice, Amnesia solution graph) | out-of-model | user picks levels; report tells them whether target k is met and what to widen |
| l-diversity entropy/recursive variants, t-closeness, β-likeness, δ-disclosure, differential privacy | out-of-model | distinct l-diversity only; others need distribution math + design surface beyond one page |
| Microaggregation / top-bottom coding / sampling | out-of-model | not built |
| Risk analysis suites (prosecutor/journalist attacker models), utility metrics | out-of-model | report covers class sizes + at-risk % only |
| File upload / repository import-export (Amnesia Zenodo etc.) | out-of-model | paste-CSV page (consistent with sibling CSV tools); CLI covers files via shell |

## UX control patterns observed → ours

- k as a small numeric setting → slider (2–100, step 1).
- Column pickers → comma-separated names/indices text fields (page has no column-picker
  widget; same pattern as csv-pii-redactor).
- Per-QI hierarchy attachment → `name:level` suffix syntax documented on the page.
- Preset/demo datasets (Amnesia ships demo data) → three `[[example]]` chips: hospital demo
  (classic age/zip/gender table, k=2), suppression demo, audit-only (report) demo.
- Preview of anonymized output + stats side by side → `output` enum `both` default (CSV then
  report in one text output).

## Non-dup note

Distinct from `csv-pii-redactor` (mask/hash/redact chosen columns — no generalization, no
k-anonymity math), `redact-pii`/`pii-tokenize` (free-text), `ip-log-anonymizer` (log lines).
This tool is the only one that generalizes quasi-identifiers and measures/reports k-anonymity.
