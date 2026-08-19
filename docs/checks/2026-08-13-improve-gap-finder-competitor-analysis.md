# gap-finder — competitor analysis (2026-08-13)

Scan run before finishing the implementation, per `create-next-tool` step 4. Findings below are
paraphrased observations of functionality and UX patterns only; no competitor copy, branding, or
trademarks are reused.

## Dup check

| Existing block | What it does | Why it is not this tool |
| --- | --- | --- |
| `numeric-range-check` | validates CSV numbers against min/max bounds | flags values outside a range, but does not enumerate missing sequence values or duplicate IDs |
| `ip-range-expand` / `ip-range-to-cidr` | expand/compress network address ranges | domain-specific IP tools, not arbitrary invoice/order/check sequences |
| `numeric-row-deduplicator` | removes duplicate numeric rows | duplicate handling only; no expected consecutive sequence or gap ranges |
| `csv-find-incomplete-rows` | finds rows with missing cells | CSV completeness, not missing IDs in a numeric run |

No existing block audits a consecutive numeric or prefixed-ID sequence for missing values, gaps, and
duplicates.

## Competitors reviewed

1. **Order ID gap finder tools** — paste order/ticket/invoice IDs, then report missing and duplicate
   identifiers. Usually focused on business identifiers rather than math sequences.
2. **Invoice numbering checkers** — emphasize legal/accounting review of sequential invoices;
   table-stakes include duplicates, gaps, and expected start/end context.
3. **Find missing numbers in a sequence tools** — accept pasted or comma-separated numbers and list
   the missing integers between the min and max.
4. **Invoice sequence auditors** — often add duplicate detection and explain that numbering runs may
   be scoped by fiscal year or prefix.
5. **General missing-number calculators** — accept an integer list and emit individual missing
   values or ranges.

## Table stakes → decisions

| Capability | Seen in | Verdict |
| --- | --- | --- |
| Paste a list of numbers or IDs | 1, 2, 3, 4, 5 | **in-model** — `data` textarea, `separator` auto/newline/comma/space/semicolon/tab/pipe |
| Missing individual values | 3, 5 | **in-model** — `output = missing`, also included in JSON |
| Compressed gap ranges | 1, 2, 4, 5 | **in-model** — report and TSV table use ranges |
| Duplicate reporting | 1, 2, 4 | **in-model** — `duplicates` checkbox with counts |
| Expected start/end overrides | 2, 4 | **in-model** — `start` and `end` catch leading/trailing gaps |
| Prefixed invoice/order IDs and zero padding | 1, 2, 4 | **in-model** — `id_format = auto` preserves shared prefix/suffix/padding |
| Non-unit step sizes | 3, 5 | **in-model** — `step` supports every 2nd/10th/etc. sequence and flags off-step values |
| Out-of-order detection | 1, 4 | **in-model** — `order = input` reports entries that go backwards |
| Machine-readable export | 1, 3 | **in-model** — `output = table` or `json` |
| File upload/import | 1, 2 | **out-of-model for this pure text page** — paste/deep-link only; file upload is not needed for the block model |
| Multi-series grouping by prefix/year | 2, 4 | **out-of-model** — mixed prefixes are rejected so users audit each series separately |
| Database reconciliation / authoritative invoice register lookup | 2, 4 | **out-of-model** — this is a local sequence audit, not a connected accounting integration |

## Notes

- The page and descriptor include every in-model table-stake above.
- Out-of-model items are called out as limits or rejected with actionable errors; none are silently
  dropped.
