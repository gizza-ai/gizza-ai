# fuzzy-dedupe — competitor analysis (2026-07-22)

Scan run BEFORE implementation to scope the tool and avoid shipping a redundant clone.

## What the tool is

Find and **merge/remove near-duplicate rows or strings** via fuzzy similarity (typos,
casing, spacing), not just exact matches, and return the **de-duplicated dataset** —
one representative kept per near-duplicate group. Pure Rust + Levenshtein, runs locally.

## Positioning vs. our existing tools (dup check)

- `blocks/csv-dedupe` — removes **exact** duplicate rows (optionally keyed on columns).
  fuzzy-dedupe is the **fuzzy** counterpart (catches typos/casing/spacing near-dups).
- `blocks/cluster-similar-values` — fuzzy-groups values in **one column** and emits a
  **canonicalization mapping** (original→canonical for every value). Its output is a
  *mapping/clusters table*, NOT a cleaned dataset. fuzzy-dedupe's primary output is the
  **de-duplicated data itself** (near-dups dropped, one row kept per group), plus a
  `keep` strategy (first / longest / most_frequent) to choose the survivor. Different
  user intent (clean my list) and different primary output (rows removed). Not a dup.

## Top competitors scanned

1. **Datablist — Free Deduplication Tool** (datablist.com/features/duplicates-remover)
   - Modes: Exact, Smart (word-order/URL protocol), Phonetic, Fuzzy (Levenshtein +
     Jaro-Winkler). Match on selected columns or full row ("Selected Properties").
   - Master-record selection: automatically keeps the "item with the most information"
     and fills gaps from secondaries. Exports a "Changes List" of updates/deletions.
   - Input CSV/Excel; up to 1M rows free / 1.5M paid. Threshold parameters not exposed
     in the free docs.
2. **DedupFuzzy** (dedupfuzzy.com)
   - Browser-based, AI/agentic matching engine; **threshold slider** for strictness;
     produces ranked candidate pairs. Consolidates duplicate groups into a "golden" row.
   - Free tier 500 rows. Company-data focused; matching engine is proprietary/AI.
3. **Remove Duplicates Online** (remove-duplicates-online.org)
   - Exact-only (no fuzzy). But strong list ergonomics: Ignore case, Trim whitespace,
     remove empty lines, custom delimiter, **keep first vs last**, sort (alpha /
     frequency / input order), views for repeated-only vs unique-only. Local, no upload.

Also noted: WinPure, ExisEcho, Flookup (`=FUZZYSIM`) — enterprise/spreadsheet-plugin.

## Table-stakes → our decisions

| Capability | Competitors | fuzzy-dedupe | In model? |
|---|---|---|---|
| Fuzzy near-dup grouping | all | ✅ normalized Levenshtein ratio (0–100) | yes |
| Similarity threshold slider | Datablist/DedupFuzzy | ✅ `threshold` 0–100 (slider), default 85 | yes |
| Match on selected columns or whole row | Datablist/DedupFuzzy | ✅ `columns` (names/1-based, blank = whole row) | yes |
| CSV + plain list input | Datablist / RDO | ✅ CSV or newline list, `delimiter`, `header` | yes |
| Case / whitespace normalization | RDO | ✅ `normalize_case`, `normalize_spacing` | yes |
| Survivor / master-record selection | Datablist (most info), RDO (first/last) | ✅ `keep` = first / longest / most_frequent | yes |
| Returns the cleaned dataset | Datablist/DedupFuzzy | ✅ `output=deduped` (primary) | yes |
| Review what was removed / groups | Datablist "Changes List" | ✅ `output=removed` and `output=json` (groups+stats) | yes |
| Jaro-Winkler / phonetic matching | Datablist | ➖ single Levenshtein metric for now (out-of-scope; note on page) | metric add-on |
| AI/agentic adjudication, "golden" merge of fields | DedupFuzzy/Datablist | ❌ out-of-model (needs ML / cross-field merge heuristics) | no |
| Multi-file / cross-dataset matching | WinPure | ❌ out-of-scope (single-input pure tool) | no |

Out-of-model / out-of-scope, stated on the page, not built: Jaro-Winkler & phonetic
metrics, AI adjudication, cross-field "golden record" merging, multi-file matching.

## Copy/branding

NONE of the competitor copy, names, or trademarks are reused. Page copy is generic and
brand-free.
