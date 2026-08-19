# faceted-filter — competitor analysis (2026-08-06)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are **paraphrased**; no competitor copy, branding, or trademark is reproduced.

## Function under study

Run **faceted search** over a pasted JSON dataset (an array of records): apply a text query
and a filter expression, then return the matching records — sorted and paginated — **plus the
facet value counts** ("Brand: Apple (12), Samsung (7)…") that drive a faceted-navigation UI.
The distinguishing deliverable versus a plain filter is the *facet count structure*, computed
from the current result set.

## Duplicate / viability check

`ls blocks/ | grep -iE 'facet|filter|search|query|json'` + `grep -rln facet blocks/*/core/src/lib.rs`
(only unrelated hits: mesh-convert, readability-score, stl-repair, swagger2-to-openapi3) +
`docs/tool-skiplist.txt` grep (no facet entry).

| Candidate | Verdict |
| --- | --- |
| `blocks/ndjson-filter` | Filters/reshapes **NDJSON lines** with a predicate + field selection. No counts, no facets, no sorting, no pagination. Adjacent predicate language (deliberately reused for consistency), different deliverable. Not a dup. |
| `blocks/jsonpath-query`, `blocks/jq-query`, `blocks/jsonata-query` | Evaluate a **path/program expression** against JSON and return the selection. No aggregation over a result set, no facet structure. Not a dup. |
| `blocks/csv-filter`, `blocks/csv-query` | CSV-shaped input (column conditions / SQL-ish). Different input model, no facet counts. Not a dup. |
| `blocks/search-index-builder` | Emits a serialized **inverted index** for a downstream search engine. Not a query-time faceted result. Not a dup. |
| `blocks/fuzzy-doc-search` | Ranked snippet search over **prose**, not structured records; no facets. Not a dup. |
| `blocks/word-frequency` | Counts word occurrences in text, not field-value counts across records. Not a dup. |

**Viable, pure-Rust:** parsing + grouping + counting over `serde_json` values; no engine crate,
no network, no model. `serde_json`'s `preserve_order` feature keeps user record key order intact
(the same feature `ndjson-filter/core` already relies on).

## Competitors reviewed

Consumer "paste JSON, get facets" web tools barely exist — the function lives in search
engines and JS libraries. Three real, reachable references define the table stakes:

### 1. ItemsJS (open-source in-browser faceted search engine)

The closest analog: a client-side library that takes an array of items and returns filtered
items + aggregations, with no server.

- Per-facet config: a display title, `size` (values returned, default 10), `sort` by count or
  key, `order` asc/desc, `conjunction` true/false (AND vs OR semantics *within* a facet),
  `hide_zero_doc_count`, `chosen_filters_on_top`, and `show_facet_stats` (min/max/avg/sum).
- Search options: `query` (full-text), `filters`, a boolean `filters_query` syntax, `sort`
  (named sortings configured as field + asc/desc, multi-field allowed), `page`, `per_page`.
- Response shape: `data.items`, `data.aggregations` keyed by field with `buckets` of
  `{key, doc_count}`, and pagination carrying `page` / `per_page` / `total`.
- **Table-stakes taken:** per-facet value cap defaulting to 10, count-vs-value facet ordering
  with a direction, disjunctive (OR-within-a-facet) counting, optional numeric facet stats,
  full-text query alongside filters, multi-field sort, page/per_page pagination, and a result
  envelope that carries items + facets + totals together.

### 2. Meilisearch (facet distribution API)

- A `facets` request parameter names which attributes to break down; the response carries a
  `facetDistribution` object of value→count per requested attribute.
- `facetStats` is returned **only for numeric facets**, and contains `min`/`max` of the current
  result set — a cheap, high-signal addition.
- Filter syntax is expression-shaped: comparisons joined with AND/OR/NOT, an `IN [a, b]`
  membership form, and `TO` for inclusive numeric ranges.
- Facet value ordering is configurable between alphabetical and by-count.
- **Table-stakes taken:** an explicit facet-field list, value→count distribution, numeric facet
  stats (min/max, plus avg/sum which cost nothing extra), an `in` membership operator for
  multi-select filtering, and alphabetical-vs-count facet ordering.

### 3. Azure AI Search (faceted navigation)

- Per-facet query parameters: `count` (max terms per facet, **default 10**), `sort`
  (`count`/`-count`/`value`/`-value`), `values` (explicit numeric/date bucket edges), `interval`
  (numeric or calendar bucketing), `timeoffset`.
- Documents faceting over **collection fields** (a tags array): one record contributes to every
  value in its array, so per-facet counts can exceed the record total.
- Documents the guidance that facets are computed from the *current* result set, that facets
  are one level deep (no hierarchy outside preview), and that a facet field with unique values
  (an id) is a poor facet.
- **Table-stakes taken:** default facet cap of 10, the four count/value × asc/desc orderings,
  array-field ("collection") faceting where each element counts separately, and stating the
  result-set-scoped nature of the counts on the page.

## Gap list → decisions (every table-stake lands in the descriptor or below)

| Capability | Source | Decision |
| --- | --- | --- |
| Filtered items + facet counts + totals in one envelope | all 3 | **In model** — `output = json` returns `total`/`page`/`per_page`/`total_pages`/`items`/`facets`. |
| Full-text `query` across records | ItemsJS | **In model** — `query` + `search_fields` (blank = every string field, case-insensitive substring). |
| Filter expression with AND/OR/NOT, comparisons, membership | Meilisearch, ItemsJS | **In model** — `filters`, reusing the repo's `ndjson-filter` predicate grammar plus an `in` operator (`brand in Apple, Sony`). |
| Explicit facet field list | Meilisearch, Azure | **In model** — `facets` (comma-separated dotted paths); **blank auto-detects** the first 20 top-level scalar/array fields, so the tool shows facets before the user configures anything (better default than all three, which require configuration). |
| Facet value cap, default 10 | ItemsJS `size`, Azure `count` | **In model** — `facet_limit` (0 = unlimited); each facet also reports `distinct` so truncation is visible. |
| Facet ordering count/value × asc/desc | Azure `sort`, ItemsJS | **In model** — `facet_sort` enum `count_desc` (default) / `count_asc` / `value_asc` / `value_desc`. Azure's `count`/`-count` spelling is ambiguous about direction; explicit names read better in a CLI and for an LLM. |
| Disjunctive (OR-within-facet) counting for multi-select UIs | ItemsJS `conjunction:false`, Algolia | **In model** — `facet_mode` enum `conjunctive` (default, counts match the shown rows) / `disjunctive` (a facet's own filter clauses are ignored when counting that facet, so unselected values keep non-zero counts). |
| Numeric facet stats | Meilisearch `facetStats`, ItemsJS `show_facet_stats` | **In model** — `facet_stats` boolean adds `min`/`max`/`avg`/`sum` for numeric facets. |
| Array/collection field faceting | Azure | **In model** — array elements each count separately (documented on the page, since counts can then exceed `total`). |
| Multi-field sort with direction | ItemsJS sortings | **In model** — `sort = "price:desc, name"`. |
| Pagination (`page` + `per_page`) | all 3 | **In model** — `page` (1-based) + `per_page` (1–1000). Out-of-range pages return empty items with the real `total_pages` rather than an error. |
| Output shaping for chat/CLI ergonomics | — (ours) | **In model** — `output` enum `json` / `items` / `facets` / `summary` (a human-readable count listing). |
| `hide_zero_doc_count` | ItemsJS | **N/A by construction** — facet values are derived from the records in scope, so a zero-count value cannot appear (conjunctive mode). Noted, not a param. |
| `chosen_filters_on_top` | ItemsJS | **Considered, rejected** — a presentation-order concern for a live sidebar widget; this tool returns data, and `facet_sort` already covers ordering. |
| Numeric/date range bucketing (`interval`, `values`) | Azure | **Out of model for v1** — a distinct histogram feature (calendar intervals, bucket-edge syntax) that would roughly double the facet surface. Listed, not built; `filters` already expresses ranges (`price >= 10 and price < 20`). |
| Hierarchical facets (`a > b > c`) | Azure preview, Algolia | **Out of model for v1** — needs a separate path-splitting facet type. Listed, not built. |
| Facet-value search (`facetQuery`) | Meilisearch, Algolia | **Out of model** — an interactive typeahead inside a facet list; there is no persistent index here. Listed, not built. |
| Relevance scoring / typo tolerance | Meilisearch, Algolia | **Out of model** — needs a ranking engine; `blocks/fuzzy-doc-search` covers ranked prose search. Listed, not built. |
| Server-side index, accounts, API keys, shards | Meilisearch, Azure, Algolia | **Out of model** — gizza tools are browser-local, no account, no backend. |

## UX patterns adopted

- **Preset chips** (`[[example]]`) for the three real workflows the competitors demo: plain
  facet breakdown, filter + facets, and disjunctive multi-select counting.
- `multiline = true` on the dataset field so pasted JSON keeps its newlines.
- Every fixed-choice knob is a `Param::enumv` → a real `<select>`, not a text box.
- Placeholders show a real dataset / real filter expression, and the page states its limits
  (50 000 records, 1 000 rows per page, 20 auto-detected facet fields, object-valued fields
  skipped) rather than leaving users to discover them via an error.

Sources: [ItemsJS](https://github.com/itemsapi/itemsjs) ·
[Meilisearch facet search](https://www.meilisearch.com/docs/learn/filtering_and_sorting/search_with_facet_filters) ·
[Azure AI Search faceted navigation](https://learn.microsoft.com/en-us/azure/search/search-faceted-navigation)
