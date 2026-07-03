# cartesian-product — competitor analysis (2026-07-03)

Scanned before implementation (one WebSearch + top-3 skims, paraphrased — no competitor
copy/branding reproduced). Tool: generate every combination (tuple) across two or more
input lists, e.g. sizes × colors × materials. Category: data, type: pure.

## Competitors skimmed

1. **dCode — Cartesian Product Generator** (dcode.fr/cartesian-product)
   - Input: multiple sets, entered one set per line in a table.
   - Actions: generate the full product, pick 1 random combination, count the cardinality.
   - Output: all pairings; export to .csv/.txt; copy-paste.
   - Limit: generation capped at 10,000 items (stated on page).
   - Worked examples: card figures {J,Q,K} × 4 suits = 12; 3 colors × 5 sizes = 15;
     4-wheel letter padlock = 26^4 = 456,976 (count only).

2. **MeFancy — Cartesian Product Generator** (mefancy.com/fun/carty)
   - Input: one text box per list, lists added dynamically; drag-drop file upload.
   - Options: join separator between the items of each combination — space, none
     (concatenation), comma+space, dash, underscore, pipe, newline, custom; optional
     prefix/suffix wrapped around each generated combination.
   - Output: combination list; copy to clipboard; download as text.
   - Limits: claims unlimited lists; no stated combination ceiling.
   - Use cases highlighted: SEO keyword permutations, product variants
     (color/size/material), test data, domain-name brainstorming, password candidates.

3. **Calculife — All Possible Combinations Generator** (calculife.com)
   - Modes: combinations, permutations, combinations-with-repetition, cartesian product
     ("pick one from each list", lists added via an Add-list button).
   - Parsing: split by auto/newline/comma/semicolon; trim spaces; ignore empty lines;
     remove duplicate items.
   - Output: separator joins items within a result; prefix/suffix per line; text or CSV
     export.
   - Limit: 5,000,000 lines per run.

(4th hit, TurboUtilKit List Combinator, returned HTTP 403 — noted from the search
snippet only: offers cartesian mode plus a "zip by index" mode.)

## Table stakes → decisions

| Capability | Competitors | Tag | Our descriptor |
|---|---|---|---|
| Multiple lists input | 1 box per list (MeFancy, Calculife); one-set-per-line table (dCode) | in-model | `list1`/`list2` required + `list3`/`list4` optional — one multiline textarea each (page form is static; 4 lists covers sizes×colors×materials + one more; chain runs for more, see FAQ) |
| Item splitting within a list | auto/newline/comma/semicolon; trim; drop empties | in-model | `item_separator` enum `auto|comma|newline|semicolon|pipe`, default `auto`; items always trimmed, blanks dropped |
| Join separator between tuple items | space, none, comma, dash, underscore, pipe, custom | in-model | `join_separator` enum `space|none|comma|dash|underscore|pipe|tab|custom` (default `space`) + `custom_join_separator` |
| Prefix/suffix per combination | MeFancy, Calculife | in-model | `prefix` / `suffix` strings (apply to `lines` output) |
| Dedupe items per list | Calculife | in-model | `dedupe` boolean, default false |
| Output formats | text list; CSV export (dCode, Calculife) | in-model | `output_format` enum `lines|csv|json` (JSON = array of per-combination arrays; CSV = one quoted row per combination) |
| Combination cap | dCode 10,000; Calculife 5,000,000 | in-model | `max_combinations` integer, default 10,000, hard cap 100,000 (browser/chat surface — error states the exact count when exceeded) |
| Count/cardinality readout | dCode count button | in-model (copy) | count = product of list sizes; stated in page copy/FAQ; the error on cap overflow reports the exact count |
| Copy / download result | all three | in-model (platform) | shared page chrome already provides Copy result |
| Unlimited dynamic "Add list" UI | MeFancy, Calculife | out-of-model (static generated form) | fixed 4 list fields; FAQ documents chaining output into list1 for >4 lists |
| Random single combination | dCode | out-of-model here | different tool shape (non-deterministic); not built |
| Zip/align-by-index mode | TurboUtilKit | out-of-model here | different semantic (pairwise zip, not a product); not built |
| Combinations/permutations modes | Calculife, Hostt | out-of-model here | separate backlog tools; not built |
| File upload / drag-drop input | MeFancy | out-of-model | page fields are text-only for pure tools; not built |

## Behavior decisions

- Tuple order: rightmost list varies fastest (odometer order, matches itertools.product
  and every competitor output skimmed).
- Empty required list (`list1`/`list2` with no items after splitting) → explicit error
  naming the list; empty optional lists (`list3`/`list4`) are simply ignored — the
  mathematical "product with the empty set is empty" reading is useless in a form UI and
  no competitor does it.
- Count computed with overflow-safe multiplication before generating; exceeding
  `max_combinations` errors with the exact count and the cap, generating nothing.
- `prefix`/`suffix`/`join_separator` shape the `lines` format; `csv` and `json` are
  structural formats (quoting/escaping owned by the format, not the join options).
