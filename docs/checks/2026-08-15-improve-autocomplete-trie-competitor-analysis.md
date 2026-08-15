# autocomplete-trie — competitor analysis (2026-08-15)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased
from public documentation/spec pages; no competitor copy, branding, or trademarks are reproduced.

Backlog row: `autocomplete-trie` — "Builds a prefix trie from a wordlist and returns ranked
autocomplete suggestions for a typed prefix." (type hint: pure)

## Duplicate check

`ls blocks/ | grep -iE 'trie|autocomplete|prefix|suggest|typeahead|fuzzy|search|match|rank'` plus
inspection of the three nearest blocks:

| Existing block | What it actually does | Verdict |
| --- | --- | --- |
| `full-text-search` | BM25/TF-IDF ranking of **documents** in a pasted corpus against a multi-term query; has a `prefix` flag that only widens term matching inside document scoring | Not a dup — ranks documents, not wordlist terms; no trie, no weights column, no structure output |
| `fuzzy-doc-search` | Ranks **snippets** (line/sentence/paragraph) of one document by bounded Levenshtein against query words | Not a dup — snippet retrieval from prose, not typeahead over a term dictionary |
| `search-index-builder` | Emits a serialized **inverted index** JSON for a static site to load; token → postings | Not a dup — an index artifact for another runtime to query; it never answers a prefix query |
| `spell-check`, `word-frequency`, `emoji-search`, `csv-column-mapping-suggest` | dictionary correction / term counting / emoji lookup / header mapping | Unrelated |

Nothing in `docs/tool-skiplist.txt` names this slug or a near-dup. **Proceeding with the build.**

## Competitors reviewed

1. **Princeton COS 226 "Autocomplete Me" assignment spec** (cs.princeton.edu) — the canonical
   weighted-autocomplete reference implementation spec.
2. **DSAVisualization trie visualizer** (dsavisualization.com/trie) — interactive prefix-tree tool.
3. **Selfboot algorithm gallery trie visualization** (gallery.selfboot.cn/en/algorithms/trie) —
   interactive prefix-tree tool with a live stored-word list.
4. **OpenGenus "Autocomplete feature using Trie"** (iq.opengenus.org) — reference article +
   implementation showing the baseline algorithm most trie-autocomplete tools ship.
5. **FuzzySearch** (github.com/jeancroy/FuzzySearch) — a production suggest-as-you-type engine;
   read for its option surface (the GitHub page 403s WebFetch, so its documented option table was
   read via search-result extraction rather than a direct fetch).

`persona500.com/trie-visualizer` returned HTTP 403 and was **replaced** by the OpenGenus reference
implementation so the scan still covers five real sources.

## Table stakes → decisions

| Capability | Seen in | Fit | Decision |
| --- | --- | --- | --- |
| Insert a wordlist and build the prefix trie | all | in-model | `wordlist` param, one term per line |
| Weighted terms (`weight` + `term` pairs) | Princeton | in-model | optional weight after a tab / comma / pipe on each line |
| Repeated terms act as frequency counts | FuzzySearch, typeahead system-design writeups | in-model | duplicate terms sum their weights (unweighted repeat = +1) |
| Top-k ranked suggestions for a prefix | Princeton, OpenGenus | in-model | `prefix` + `limit` (1–100, default 10) |
| Rank strictly by weight desc, ties broken deterministically | Princeton (ties unspecified — a real gap) | in-model | `rank = weight` default; ties break alphabetically so output is stable |
| Alternative orderings (alphabetical / shortest first) | trie visualizers list words alphabetically | in-model | `rank = alphabetical \| shortest` |
| Case handling | OpenGenus (lowercases silently) | in-model | `case_sensitive` (default off); original casing preserved in output |
| Report total match count, not just the top-k page | Princeton (`numberOfMatches`) | in-model | text caption + JSON `matches` field |
| Empty / not-found prefix behaviour | OpenGenus returns "NOT FOUND" | in-model | empty prefix = top terms overall; no match = explicit "no suggestions" message |
| Typo tolerance during the trie walk | FuzzySearch; system-design writeups ("bounded edits during the trie walk") | in-model (trie + DP row) | `max_typos` 0–2; exact-prefix hits always outrank fuzzy hits |
| Trie structure visualisation (shared prefixes, terminal nodes) | DSAVisualization, Selfboot | in-model | `output = trie` renders the matched subtree as an ASCII tree with `*` terminal markers |
| Trie statistics (stored words, node count, depth) | DSAVisualization ("Stored Words"), Selfboot | in-model | stats line on every text/trie run; `stats` object in JSON |
| Shared-prefix compression savings | implied by both visualizers' "reused node" explanation | in-model | reported as characters stored vs. characters pasted |
| Structured output for scripting | FuzzySearch (score + item + match details) | in-model | `output = json` |
| Preset examples | all interactive tools ship a "random initialize" or seeded list | in-model | five `[[example]]` chips on the page |
| Animated step-by-step insert/search playback | DSAVisualization, Selfboot | **out-of-model** | gizza blocks return a value, not a timeline; a static structure dump is the honest equivalent (shipped as `output = trie`) |
| Interactive delete / clear operations on a persisted trie | DSAVisualization, Selfboot | **out-of-model** | blocks are stateless request→response; there is no trie to mutate between calls. Removing a term = deleting its line and re-running |
| Multi-field / multi-document scoring, field boosts, highlight markup | FuzzySearch | **out-of-model for this tool** | that is corpus search, already served by `blocks/full-text-search` (BM25 + field/title boost) and `blocks/fuzzy-doc-search` (snippet highlighting). Duplicating it here would fragment the search surface |
| Learning from user selections (feedback loop that increments counts) | typeahead system-design writeups | **out-of-model** | requires cross-call persistence |

Every table stake above lands in the descriptor or in the out-of-model list; none was dropped
silently.

## Resulting descriptor

`wordlist` (required, multiline) · `prefix` · `limit` (1–100, default 10) ·
`rank` (`weight|alphabetical|shortest`) · `max_typos` (0–2) · `case_sensitive` ·
`output` (`text|json|trie`).

## Notes

- No competitor copy or naming was reused; page copy is generic and brand-free.
- Out-of-model items are listed here only, and the page states the same limits in its FAQ.
