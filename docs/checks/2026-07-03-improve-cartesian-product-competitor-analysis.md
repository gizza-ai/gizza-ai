# cartesian-product — competitor analysis (2026-07-03)

Two passes, same day. **Build pass** (kept below for the record): one WebSearch +
top-3 skims before implementation. **Improve pass**: extended to the top 5 real
competitors, one read-only research subagent each, deeper cuts
(params/defaults/output/UX/SEO, limits, free-vs-paid). All paraphrased — no
competitor copy/branding reproduced. Tool: generate every combination (tuple)
across two or more input lists, e.g. sizes × colors × materials. Category: data,
type: pure.

## Competitors profiled (improve pass, top 5)

1. **dCode — Cartesian Product Generator** (dcode.fr/cartesian-product)
   - Input: multiple sets in one table, one set per line; newline is the only
     element separator.
   - Actions: generate the full product; pick 1 random combination; a
     cardinality button that multiplies the set sizes WITHOUT generating.
   - Output: tabular grid; export .csv / .txt; copy to clipboard.
   - Limits: caps generation at 10,000 items (silently caps, per profile); no
     documented empty-set/dedupe handling.
   - SEO: definition-led educational copy; worked examples around playing-card
     figures × suits, garment colors × sizes, multi-wheel lock codes; FR/EN.
   - Positioning: free, no account, CC-BY content, no paid tier.

2. **MeFancy — Carty, Cartesian Product Generator** (mefancy.com/fun/carty)
   - Input: one textarea per list, unlimited lists via an add-list button;
     drag-and-drop a text file into a list.
   - Options: join separator between tuple items — space (default), none
     (concatenate), comma+space, dash, underscore, pipe, **newline**, custom;
     prefix/suffix around each combination.
   - Output: text; copy; download as file; **live combination counter**.
   - Limits: none stated; claims unlimited lists.
   - SEO: SEM keyword variations, e-commerce variants, test data, domain
     ideation, credential-testing lists.
   - Positioning: free, no account, no visible ads.

3. **Calculife — All Possible Combinations Generator**
   (calculife.com/all-possible-combinations-generator/)
   - Modes: combinations / permutations / combinations-with-repetition /
     cartesian product; result length all-lengths or exact-k.
   - Parsing: delimiter auto|newline|comma|semicolon; trim whitespace,
     remove duplicates, ignore empty lines — each a toggle (default on).
   - Output: separator/prefix/suffix; .txt or .csv (one column per list in
     cartesian mode); shows BOTH the theoretical total and the capped
     "will generate" count; generate/pause/resume/stop batch controls with a
     live scrolling preview.
   - Limits: 5,000,000 lines per run (stated browser-stability cap).
   - SEO: combinations-vs-permutations education; toppings/digits/inventory
     examples; spreadsheet-export angle.
   - Positioning: free, no account, no ads.

4. **Toptal — Merge Words** (toptal.com/marketing/mergewords)
   - Input: exactly three word lists (fixed count); words split on spaces AND
     newlines; quoted phrases and search operators survive.
   - Options: separator nothing|space|minus|plus|custom (default newline
     between words); wrap each combination in nothing|double quotes|square
     brackets (PPC match types); collapsible "extra options" panel.
   - Output: one combination per line in a copy-paste textbox; **live counter**
     of the total.
   - Presets: three one-click templates (domain registration, linkbuilding
     search operators, paid-search keywords).
   - Limits: 3 lists only; no stated output cap.
   - Positioning: free utility maintained by a services marketplace; no ads,
     no account.

5. **List Combinator** (listcombinator.com)
   - Input: 3 list slots to start, add/reorder/delete lists dynamically;
     delimiters comma/pipe/semicolon/slash/tab/newline with auto-detect and a
     mixed-delimiter conversion helper; blank-row cleanup; per-list dedupe.
   - Options: custom inter-item joiner (may be empty); per-list prefix/suffix.
   - Output: in-browser render, copy, or direct file download for large sets;
     estimated combination count shown BEFORE generation; shareable URL.
   - Limits: none hard-stated; warns about exponential growth and recommends
     the file-download path for 10k+ combinations.
   - SEO: 17-language UI; keyword research, product naming, taxonomy trees,
     batch prompt/test-case generation, naming conventions.
   - Positioning: free, ad-supported, no account.

## Gap list (improve pass) — 4 dimensions, tagged

### Capabilities
| Gap | Who has it | Tag | Decision |
|---|---|---|---|
| Count/cardinality without generating | dCode (button), Calculife (dual counts), MeFancy/MergeWords/ListCombinator (live counters) | in-model | **Built:** `output_format = "count"` → `"2 x 3 = 6"`, exempt from `max_combinations` (sizing up a too-big product is the point). Error on cap overflow already reported the exact count. |
| Tab as item separator (spreadsheet paste) | List Combinator (tab delimiter) | in-model | **Built:** `item_separator = "tab"`, splits tabs AND newlines so a 2-D cell block pastes as one item per cell; auto-detect tries tab first. |
| Slash as item separator | List Combinator | in-model | **Built:** `item_separator = "slash"`, explicit-only (never auto-detected — URLs/dates keep their slashes). |
| Newline as join separator | MeFancy, MergeWords (default) | in-model | **Built:** `join_separator = "newline"` (it earns an enum slot because a newline cannot be typed into the custom-join field; `plus`/`minus` etc. stay custom-typable). |
| Quote/bracket wrapping (PPC match types) | MergeWords | in-model | **Covered by existing `prefix`/`suffix`** — new example chip + copy show the `"..."` phrase-match and `[...]` recipes; no new param. |
| Download result as file | all five | in-model (platform) | **Built platform-wide:** `format = "text"` tools now render a Download link next to Copy result (generator template + shared tool.js blob sync). Every text tool gains it. |
| Per-list prefix/suffix | List Combinator | in-model, rejected | Not built: +8 params of chat-schema and form bloat for a niche wrap; achievable by editing list items or chaining runs. Listed for the reviewer to veto. |
| 5M-line outputs | Calculife | out-of-model here | Our 100k hard cap is a deliberate browser-tab/chat-surface guard; count format now covers sizing bigger products. |
| Pause/resume/progress batching | Calculife | out-of-model here | Unnecessary under the 100k cap — the wasm run is instant. |
| Random single combination | dCode | out-of-model here | Non-deterministic output is a different tool shape (random-picker); breaks exact-output testing model. Backlog candidate. |
| Zip/align-by-index mode | (TurboUtilKit, build scan) | out-of-model here | Different semantic; separate backlog tool. |
| Combinations/permutations/k-subsets | Calculife, Hostt | out-of-model here | Separate backlog tools. |
| File upload / drag-drop lists | MeFancy | out-of-model | Pure-tool pages are text-field driven; no file plumbing for pure tools. |
| Search-volume enrichment | Umbrellum (search hit) | out-of-model | Needs a server/API + data licence. |

### Copy + SEO
- Spreadsheet-paste angle (column/row/2-D block) — **added** (feature bullet + FAQ).
- Count-before-you-generate angle with a big-number worked example (26⁴ =
  456,976 — original numeric example, not dCode's padlock story) — **added**.
- PPC wrap recipes (`"..."`, `[...]`, `+broad`) — **added** to the join bullet.
- Synonym tags competitors rank for: word combiner, keyword mixer, merge word
  lists, combination counter — **added** to meta tags.
- Numbers-stay-verbatim reassurance (`007`, `1.50`) — **added** to limits copy.

### UX / layout
- Live always-visible combination counter (MeFancy/MergeWords/ListCombinator):
  the count capability ships as an output format + chip; an ambient badge would
  be a new platform output-chrome feature — deferred, listed as follow-up.
- Friendly `<select>` labels (`[input.labels]`): "Tab — spreadsheet cells",
  "Count only (never generates)", "Nothing (concatenate)" etc. — **added**.
- Example chips extended: count-first chip + quoted-PPC chip — **added**.
- Tag-list controls for the four list fields — **evaluated, kept textareas**:
  (1) bulk paste (spreadsheet columns/rows) is the dominant workflow and pills
  force item-at-a-time entry; (2) the tag-list widget is backed by a hidden
  comma-joined input, so items containing commas become unrepresentable and
  would re-split — exactly the value-mangling class the ffmpeg passes hit;
  (3) `item_separator = newline|semicolon|pipe` exists precisely so items may
  contain commas — a comma-joined control would hardwire the one separator the
  user may be escaping. Right control for THIS data = multiline textarea.
- `max_combinations` as a slider — evaluated, kept the number box: a 1–100,000
  range thumb is imprecise where users type order-of-magnitude values.

### Visual design
- Page uses the shared gizza-chrome (cards, chips, dev-group). No competitor
  visual ideas worth importing beyond the counter (covered above); no gaps
  actioned this pass.

---

# Appendix — build-pass record (top-3 skim, pre-implementation)

Scanned before implementation (one WebSearch + top-3 skims, paraphrased — no
competitor copy/branding reproduced).

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

## Table stakes → decisions (build pass)

| Capability | Competitors | Tag | Our descriptor |
|---|---|---|---|
| Multiple lists input | 1 box per list (MeFancy, Calculife); one-set-per-line table (dCode) | in-model | `list1`/`list2` required + `list3`/`list4` optional — one multiline textarea each (page form is static; 4 lists covers sizes×colors×materials + one more; chain runs for more, see FAQ) |
| Item splitting within a list | auto/newline/comma/semicolon; trim; drop empties | in-model | `item_separator` enum `auto\|comma\|newline\|semicolon\|pipe`, default `auto`; items always trimmed, blanks dropped |
| Join separator between tuple items | space, none, comma, dash, underscore, pipe, custom | in-model | `join_separator` enum `space\|none\|comma\|dash\|underscore\|pipe\|tab\|custom` (default `space`) + `custom_join_separator` |
| Prefix/suffix per combination | MeFancy, Calculife | in-model | `prefix` / `suffix` strings (apply to `lines` output) |
| Dedupe items per list | Calculife | in-model | `dedupe` boolean, default false |
| Output formats | text list; CSV export (dCode, Calculife) | in-model | `output_format` enum `lines\|csv\|json` (JSON = array of per-combination arrays; CSV = one quoted row per combination) |
| Combination cap | dCode 10,000; Calculife 5,000,000 | in-model | `max_combinations` integer, default 10,000, hard cap 100,000 (browser/chat surface — error states the exact count when exceeded) |
| Count/cardinality readout | dCode count button | in-model (copy) | count = product of list sizes; stated in page copy/FAQ; the error on cap overflow reports the exact count |
| Copy / download result | all three | in-model (platform) | shared page chrome already provides Copy result |
| Unlimited dynamic "Add list" UI | MeFancy, Calculife | out-of-model (static generated form) | fixed 4 list fields; FAQ documents chaining output into list1 for >4 lists |
| Random single combination | dCode | out-of-model here | different tool shape (non-deterministic); not built |
| Zip/align-by-index mode | TurboUtilKit | out-of-model here | different semantic (pairwise zip, not a product); not built |
| Combinations/permutations modes | Calculife, Hostt | out-of-model here | separate backlog tools; not built |
| File upload / drag-drop input | MeFancy | out-of-model | page fields are text-only for pure tools; not built |

## Behavior decisions (build pass)

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
