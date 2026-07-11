# list-set-diff — competitor analysis (2026-07-10)

Tool: compare two lists as sets and report **only-in-A**, **only-in-B**, and **shared**
items with counts, plus normalization options. Pure-compute (no I/O).

## Competitors scanned

1. **CompareLists.org** — panels: items in A only / B only / both (intersection) / union
   (all combined). Options: Ignore case, Trim whitespace, Remove duplicates, Ignore leading
   zeros; Sort order (Original / A-Z / Z-A); per-list separator (line/tab/comma/semicolon/
   period/quotes/pipe); Example + Clear buttons; file upload; export/share.
2. **CompareTwoLists.com** — shows unique + shared values; per-list item counters; file
   upload (≤15 MB); "Case insensitive" toggle; auto-sorts, removes duplicates + empty lines;
   delimiter selection (line/tab/comma/semicolon/period/quotes/pipe); regex replace helper;
   "reduce output for large lists" download mode.
3. **IPVoid — Compare Two Lists** — narrower: only "List B lines not found in List A"
   (one-directional), with a new-line counter; geared to IPs/domains/hashes; notes browser
   freeze on 10k+ lines.
4. **ListDiff.com / DiffLists.com / OnlineTextCompare** — the common shape: paste List A +
   List B, get A-only / B-only / both panels, all in-browser, no upload.

## Table-stakes → decision

| Capability | In model? | Where it lands |
| --- | --- | --- |
| Only-in-A / only-in-B / shared panels | ✅ | core output sections |
| Per-section + summary counts (incl. union) | ✅ | `Totals:` line |
| Ignore case | ✅ | `ignore_case` (default false) |
| Trim whitespace | ✅ | `trim` (default true) |
| Ignore blank/empty lines | ✅ | `ignore_blank` (default true) |
| Remove duplicates (set semantics) | ✅ | `dedupe` (default true) |
| Ignore leading zeros (numeric IDs) | ✅ | `ignore_leading_zeros` (default false) |
| Sort order (input / A-Z / Z-A) | ✅ | `sort` enum (default input) |
| Delimiter/separator choice | ✅ | `separator` enum (newline/comma/tab/semicolon/pipe/space) |
| Example / preset chips | ✅ | `[[example]]` chips on the page |
| Union list panel (full listing) | ➖ trimmed | union is counted in `Totals:`; A-only+B-only+shared already reconstruct it — a 4th full panel is redundant, so counted not listed |
| File upload (paste a file's contents) | ➖ | out of model for the pure chat/CLI surface; the page textarea accepts paste; not built |
| Regex find/replace pre-clean | ➖ out-of-model | a separate transform; covered by the existing `list-converter` / find-replace tools, not bundled here |
| One-directional (B-not-in-A only) mode | ➖ | our bidirectional output already contains B-only; a mode toggle adds nothing |

No competitor copy, branding, or trademarks were reproduced. Out-of-model items above are
listed, not built.
