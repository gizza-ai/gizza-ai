# combinations-permutations — competitor analysis (2026-09-25)

Scan run **before** implementing, per `/create-next-tool` step 4. All competitor notes are
**paraphrased observations of behaviour**; no competitor copy, branding, or trademarks were
reproduced. "Out-of-model" items are listed, never built.

## Competitors reviewed

| # | Tool | Shape | What it does |
|---|------|-------|--------------|
| 1 | calculator.net — permutation & combination calculator | counter | Two fields (`n`, `r`, defaults 6 and 2). Prints nPr and nCr side by side with the factorial formula above each result. Documents the with-replacement formulas in prose but does **not** compute them. No enumeration, no stated max n. |
| 2 | dcode.fr — "n choose k" combinations generator | enumerator | Generates the actual list. Item source can be digits `1..n`, letters `A, B, C…`, or a custom pasted list. Toggle to drop duplicate combinations. Output items joined by a chosen separator (comma, dash, space, pipe, slash, underscore, plus, dot); export as CSV or TXT. Hard cap in the low thousands of rows ("prevents server overload"); anything larger is a paid request. Sibling pages for permutations and for combinations-with-repetition. FAQ covers counting algorithms, lottery odds, the `0 choose 0 = 1` edge case, and reference Python/JS snippets. |
| 3 | omnicalculator — combination calculator | counter + short enumeration | `n` and `r` plus a with/without-repetition switch. Shows the formula, the count, and **lists the actual combinations up to ~300 results / 10 elements**, then warns the output gets long. Cross-links permutations and walks through a lottery probability example. |
| 4 | statsmasters — nCr/nPr calculator | counter | One `n`, one `r`, and a 4-way type selector: combination, permutation, combination with repetition, permutation with repetition. Shows a **step-by-step substitution** plus the formula for the selected type (`n!/(r!(n−r)!)`, `n!/(n−r)!`, `n^r`, `C(n+r−1, r)`). Reference values such as C(52,5) = 2,598,960 and lookup tables to n = 52. |
| 5 (context) | novatoolshub — permutation & combination calculator | counter | Same four types plus **circular permutations**; advertises exact results to n = 200. |

## Table stakes → decisions

| Capability seen | Where it lands |
|---|---|
| nCr and nPr counts from `n`/`r` | **In model** — `n`, `r`, `mode` params. |
| With-repetition variants (`n^r`, `C(n+r−1, r)`) | **In model** — `repetition` boolean, applies to both modes. |
| Circular permutations | **In model** — `mode = circular_permutations`, counted as `C(n,r)·(r−1)!` and *also* enumerable (fix the first chosen item, permute the rest) — better than the counter-only competitors. |
| Formula + step-by-step substitution | **In model** — `output_format = summary` (the default) prints the formula, the substituted form, the result, and the order/repetition state. |
| Lottery-style odds ("1 in 2,598,960") | **In model** — the summary carries an odds line; it is the single most common reason people open these pages. |
| Enumerating the real combinations of a list | **In model** — `items` + `output_format = lines/csv/json`. This is the axis where the counters (1, 4, 5) have nothing. |
| Item source = digits `1..n` when no list is pasted | **In model** — enumeration with an empty `items` falls back to `1 … n`, matching dcode's digit source. Letters are one paste away (`a, b, c, d`) and ship as a preset chip. |
| Separator choice for joined output | **In model** — `join_separator` covers comma/space/none/dash/underscore/pipe/slash/plus/dot/custom (dcode's full set). |
| CSV export shape | **In model** — `output_format = csv` emits RFC-4180 rows; the page's built-in Download button covers the "export as file" need without a server. |
| JSON output | **In model** — `output_format = json` (array of arrays); no competitor offers it, and it is the natural chat/CLI shape. |
| Duplicate handling in the pool | **In model** — `dedupe` collapses repeated pool items before generating. |
| Result cap with a clear message | **In model** — `max_results` (default 10 000, hard cap 100 000), an order of magnitude past omnicalculator's 300 and past dcode's free tier; exceeding it reports the exact count instead of truncating silently. |
| Exact big-integer counts well past 64-bit | **In model** — a small base-1e9 bignum in `core`, so `C(1000, 500)` prints all 299 digits. Competitors stop at n = 170–200. |
| Lookup tables (C(52,5) etc.) | **Copy, not capability** — the page states worked reference values (poker hands, 6/49 lottery) in the content and as preset chips rather than shipping a static table. |
| CSV/TXT **file** download | Covered by the platform — `format = "text"` pages get a Download link for free. |

## Considered, not built

- **Paid/bulk server generation** (dcode's above-cap tier) — out of model: gizza is browser-local
  with no backend and no accounts. The honest answer is the `max_results` cap plus an error that
  names the real count.
- **Necklace/bracelet counting** (circular arrangements *with* repetition, i.e. Burnside/Pólya
  counting) — out of scope for this tool's schema; `circular_permutations` + `repetition` is
  rejected with an explicit message rather than silently returning a wrong number.
- **Multiset permutations of a pool with repeated items** ("MISSISSIPPI" style `n!/∏kᵢ!`) — a
  genuinely different input model (counts per symbol); `dedupe` covers the common intent.
  Noted as a possible future sibling tool, not forced into this schema.
- **Step-by-step factorial expansion printed digit by digit** (`10! = 3628800`, then the division)
  — the summary shows the substituted formula, which is the load-bearing part; printing every
  intermediate factorial for n = 200 would be noise.
- **Static lookup tables to n = 52** — SEO filler; the calculator computes any of them instantly.

## UX patterns adopted

- Preset chips (`[[example]]`) for the scenarios every competitor uses as its worked example:
  lottery 6/49, poker hands C(52,5), 3-digit PIN with repetition, listing the combinations of a
  small letter pool, and a round-table seating (circular).
- Friendly `<select>` labels via `[input.labels]` so "combinations" reads as "Combinations —
  order does not matter".
- Every text/number field carries a real placeholder; limits (max n/r 1000, cap 100 000,
  enumeration needs a pool or `n`) are stated on the page, not only in error strings.
