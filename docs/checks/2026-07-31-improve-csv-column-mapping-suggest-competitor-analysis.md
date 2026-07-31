# Competitor analysis — csv-column-mapping-suggest (2026-07-31)

Function: given two CSVs with differently-named columns, suggest which source column maps
to which target column, using header-name similarity + value overlap, so the files can be
aligned before a diff/join.

One WebSearch ("automatic CSV column mapping ... header and value similarity"); skimmed the
top related tools/write-ups (paraphrased — no copy/branding reproduced):

## Related tools / approaches skimmed

1. **CSVBox column-mapping (blog.csvbox.io)** — import-schema mapper. Auto-maps incoming
   headers to a target schema with heuristics (exact/normalized name match + synonym
   lists), then exposes a manual remap UI when uncertain. Table-stakes: normalized header
   matching (case/punctuation-insensitive), a confidence notion (auto vs "needs review"),
   one-to-one mapping to a destination schema, leftover/unmapped surfaced for manual fix.

2. **DataFlowMapper (dataflowmapper.com/blog)** — AI data-mapping. Analyzes headers, data
   TYPES, and sample VALUES to predict field connections; positions value/type inspection
   as the differentiator over pure name matching. Table-stakes: use sample values not just
   names; rank suggested mappings.

3. **"Align CSV columns even when names don't match" (Medium, difflib approach)** — fuzzy
   header matching with Python `difflib.SequenceMatcher`, ignoring special chars + case,
   producing a similarity SCORE per candidate and picking the best above a cutoff.
   Table-stakes: fuzzy (not exact-only) header score, a tunable cutoff/threshold, best-match
   selection.

Also noted: Flatfile (ML column-matching, hosted), CSV Column Mapper (csvtojsonconverter —
manual drag remap), SplitForge merge (auto-align by header on concat).

## Table-stakes → decision (in-model = browser-local wasm; out = needs server/account/ML)

| Capability | Decision |
| --- | --- |
| Normalized header matching (lowercase, strip punctuation, split camelCase/underscores) | in-model — built |
| Fuzzy header similarity + numeric SCORE (not exact-only) | in-model — built (token Jaccard ∪ char-bigram Dice) |
| Value-overlap from sample rows (distinct-value Jaccard) | in-model — built (`sample_rows`) |
| Tunable header-vs-value weighting | in-model — built (`header_weight`) |
| Confidence threshold / cutoff below which a column is left unmapped | in-model — built (`threshold`) |
| One-to-one (greedy) assignment so each target is used once | in-model — built |
| Surface unmapped source columns for manual review | in-model — built (unmapped list) |
| Machine-readable output (JSON) + human table + flat CSV mapping | in-model — built (`format`) |
| Reason/explanation per suggestion (why matched) | in-model — built (header/value contributions) |
| Delimiter / header-row controls | in-model — built (`delimiter`, `header`) |
| Synonym dictionary ("phone"↔"mobile", "zip"↔"postal") | in-model but **rejected for v1** — a curated synonym list is scope/locale creep; the bigram+token score already catches morphological variants (zip/zipcode, phone/phonenumber). Listed as a future option, not built. |
| Interactive drag-drop remap UI / persist mappings / apply-to-import | out-of-model — needs a stateful import pipeline/backend; this tool SUGGESTS a mapping, the user applies it in the diff/join tools |
| ML/embedding semantic matching (Flatfile-style) | out-of-model — needs a hosted model |

## Our design

Params: `source`, `target` (required CSV texts), `delimiter` (comma/tab/semicolon/pipe),
`header` (bool, default true), `sample_rows` (int, default 50, 0 = header-only),
`header_weight` (0–1, default 0.6), `threshold` (0–1, default 0.3),
`format` (table/json/csv, default table). Header score = max(token-set Jaccard,
character-bigram Dice) on normalized headers; value score = Jaccard of the distinct sampled
value sets; combined = weighted blend (header-only when value data is absent). Greedy
one-to-one assignment above threshold; leftover source columns reported as unmapped.
