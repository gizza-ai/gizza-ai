# http-headers-diff — competitor analysis (2026-07-26)

Scan of real "compare / diff two sets of HTTP headers" tools before building. All
findings paraphrased; no competitor copy, branding, or trademarks reproduced.

## Tools reviewed

1. **Spoold — curl Compare** (`spoold.com/tools/http/curl/compare`) — pastes two curl
   commands, parses out method/URL/query/headers/auth/body and shows a structured
   side-by-side table (`Header | A | B`), with a raw line-diff fallback when the
   structured parse is incomplete.
2. **Proxyman — Diff view** (desktop) — diffs two captured requests/responses by URL,
   method, status code, headers, and body; visual side-by-side.
3. **Tom Anthony — Bulk HTTP Header Response Checker** — fetches a list of URLs (twice
   with different user-agents) and compares the *response headers + redirect/status* to
   surface cloaking; header-set comparison, not a rich per-value diff.
4. **API Diff Online / wtools.io / DiffNow** (general) — GitHub-style unified/side-by-side
   text diff; header comparison is just line-diffing the pasted text.

## Table-stakes → in-model / out-of-model

| Capability | Verdict | How we cover it |
|---|---|---|
| Report headers **added / removed / changed** (old → new) | in-model | Core diff into 4 buckets + count summary (`report`/`json`). |
| **Case-insensitive** header-name matching (RFC 9110 names are case-insensitive) | in-model | Names always folded to lower for matching, displayed canonical Title-Case. |
| **Multi-value / repeated** headers folded before compare (RFC 9110 §5.3) | in-model | Repeated names joined with `, `; **Set-Cookie kept as a list** (RFC 6265, never comma-joined). |
| **Ignore noise headers** (Date, Age, Report-To, request-id, …) | in-model | `ignore` param: comma/space/newline list of names to drop from the diff (case-insensitive). |
| **Order-independent** compare of list-valued headers (Vary, Cache-Control, Accept) | in-model | `ignore_order` bool: compares the comma-token *set*, so reordering isn't flagged. |
| Structured **table** (Header \| A \| B) | in-model (as text) | `report` groups Added/Removed/Changed/Unchanged; `json` gives a machine object. |
| **JSON / machine-readable** output | in-model | `output = json`. |
| Tolerate a leading **request/status line** in a pasted block | in-model | An optional leading `GET … HTTP/1.1` / `HTTP/1.1 200` line is detected and skipped (not diffed). |
| Tolerate CRLF, obsolete line-folding, blank-line head/body split | in-model | Parser normalizes CRLF, joins folded continuations, stops at the first blank line. |
| **Paste curl commands** and extract `-H` headers | out-of-model | We diff header *blocks* (one `Name: value` per line), not shell command lines — a curl parser is a separate concern. Noted on the page. |
| **Fetch two live URLs** and diff their responses | out-of-model | This is a pure, no-network tool; fetching is `http-request` / a network tool's job, not this one. |
| Rich **side-by-side visual diff UI / syntax highlighting** | out-of-model | The generic tool page renders text; a bespoke two-column diff widget is a UI concern beyond the block. Grouped report is the substitute. |
| Body / status-code diff | out-of-model | Scope is header sets only, matching the tool description. |

Every table-stake lands in the descriptor or is listed above as out-of-model — none dropped
silently. Design mirrors the sibling `ini-env-diff` (report/json buckets) with HTTP-specific
parsing borrowed from `http-header-parser` (case folding, multi-value combine, Set-Cookie,
start-line detection).

## UX / controls to match

- Two large paste areas (left/old, right/new) — `multiline` textareas.
- Preset **example chips** (competitors ship "try this" samples): a security-header change, a
  cache-header change, and an ignore-order example.
- `output` and the two toggles rendered as native `<select>`/checkboxes via the manifest.
