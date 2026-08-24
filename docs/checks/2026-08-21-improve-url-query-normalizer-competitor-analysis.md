# url-query-normalizer — competitor analysis (2026-08-21)

Scope: one WebSearch for the function ("online URL query parameter normalizer sort dedupe
canonical URL tool"), then the top real tools skimmed. All notes below are **paraphrased
observations of behaviour and option surface** — no competitor copy, branding, trademarks or
marketing text is reproduced or reused anywhere in this block.

## Duplicate check (done before any implementation)

Nearby blocks inspected: `url-cleaner`, `url-stripper`, `query-string-codec`,
`parse-query-string`, `url-encode`, `parse-uri`, `url-trailing-slash-normalizer`,
`utm-link-builder`.

| Existing block | What it does | Overlap verdict |
| --- | --- | --- |
| `url-cleaner` | Drops tracking params (`utm_*`, `fbclid`, `gclid`, …). `core/src/lib.rs` header states it preserves "the remaining query params in their original order and encoding — values are never re-encoded"; test `preserves_fragment_and_order_and_encoding` pins that. | **Not a dup.** It deliberately never sorts, never dedupes, never touches encoding — the three things this tool exists to do. Tracking-drop is one optional flag here, the whole product there. |
| `query-string-codec` | Query string ↔ JSON, with array styles (brackets/indices/repeat/comma). | Not a dup — a converter to another format, not a URL→URL canonicalizer. |
| `parse-query-string` | Query string → ordered key/value pairs + structured JSON. | Not a dup — read-only inspection output, does not emit a normalized URL. |
| `url-encode` | Percent-encodes/decodes arbitrary text. | Not a dup — a codec over free text, no query-pair structure. |
| `parse-uri` | Splits a URI into components. | Not a dup. |
| `url-trailing-slash-normalizer` | Normalizes the trailing slash on the *path*; explicitly copies the query byte-for-byte. | Complementary — the path half of canonicalization; this tool is the query half. |
| `utm-link-builder` | Builds tagged campaign URLs. | Opposite direction. |

Conclusion: build it, scoped tightly to the **query string** (the tool's name). Path/host-level
canonicalization (lowercase host, default-port removal, dot-segment resolution, trailing slash)
is deliberately left to the existing blocks rather than duplicated here.

## Competitors skimmed

1. **iotools.cloud — URL canonicalizer** (search snippet only; the page itself returned HTTP 403
   to a direct fetch, so this row is from the search result summary and is marked as such).
   Configurable normalization steps: lowercase scheme and host, remove default ports, sort query
   parameters, resolve dot segments. Side-by-side before/after with diff highlighting.
2. **monocalc.com — URL canonicalizer** (fetched). Three canonicalization *profiles* (a
   standards-baseline one, an SEO one that adds param sorting + fragment removal, and a
   security/pentest one that fully decodes). Separate toggles for sort query params, remove
   duplicates ("keeps only the first occurrence"), remove fragment, decode unreserved, plus a
   trailing-slash mode (keep/add/remove) and an optional base URL for relative input. Percent
   handling documented as: decode the unreserved set (`A–Z a–z 0–9 - _ . ~`), re-encode what must
   be encoded, uppercase the hex digits. Batch tab capped at ~100 newline-separated URLs, results
   exportable as a CSV table. Worked example given as `?b=2&a=1&b=3` → `?a=1&b=2`.
3. **chunkymunster.com — URL normalizer** (fetched). Bulk-first: one URL per line (also accepts
   space/comma separated), preset output formats, and toggles for preserve path & query, remove
   duplicates, sort A–Z, lowercase. Copy-all and download-.txt on the result. FAQ covers why
   normalize at all, whether alphabetical param order changes server responses, dot-segment
   handling, and a privacy answer (nothing is uploaded).
4. Also seen in the result list and used only to confirm the table stakes are consistent:
   urltoolkit.com (lowercase host, drop default ports, sort params, dedupe URLs),
   onedev.tools (lowercase hostname, sort params, remove defaults),
   webtexttools.com (bulk clean + dedupe + sort with TXT/CSV/JSON/XLSX export).

## Table stakes → decision

| Capability | Competitors | In model here? | Shipped |
| --- | --- | --- | --- |
| Sort params alphabetically | all | in-model | `sort = none \| key \| key-value` (default `key`, stable) |
| Deduplicate repeated params | all (usually "keep first") | in-model | `dedupe = none \| exact \| first \| last` — **default `exact`**, see below |
| Percent-encoding normalization (uppercase hex, decode unreserved) | monocalc, iotools | in-model | `encoding = normalize \| preserve` (default `normalize`) |
| `+` vs `%20` for spaces | implied, rarely exposed | in-model | `space = percent \| plus` (default `percent`) — **gap we close**: none of the four exposed this as a first-class choice |
| Drop tracking params | url cleaners generally | in-model | `drop_tracking` (off by default; the `utm_*`/`fbclid`/`gclid` families) |
| Custom param allow/deny list | rare | in-model | `drop_params` + `keep_params`, both accepting a `prefix_*` wildcard — **gap we close** |
| Drop empty-valued params | rare | in-model | `drop_empty` |
| Bulk / one-per-line input | chunkymunster, webtexttools, monocalc (~100 cap) | in-model | one per line, 20,000 lines / 1,000,000 bytes — a higher cap than any competitor's |
| Bare query string (no scheme/host) as input | not seen | in-model | accepted and returned without a `?` — **gap we close** |
| CSV report / summary of what changed | monocalc (CSV export), diff view | in-model | `output = urls \| changed \| report \| summary` |
| Runs client-side, nothing uploaded | all claim it | in-model | true here (WASM in-page; the CLI is offline too) |
| Lowercase scheme/host, drop default port, resolve dot segments | monocalc, iotools, onedev | **out of scope, deliberately** | host/path canonicalization is a different axis; `url-trailing-slash-normalizer` owns the path half. Noted, not built. |
| Remove fragment | monocalc | **out of scope** | fragments are preserved byte-for-byte; dropping one is not query normalization |
| Relative-URL resolution against a base URL | monocalc | **out of scope** | needs a full URL resolver, unrelated to query params |
| Fully-decode "pentest" profile | monocalc | **out of scope** | produces strings that are no longer valid URLs; `parse-query-string` already shows decoded pairs |
| XLSX/spreadsheet export | webtexttools | **out of model** | no spreadsheet writer in a pure block; CSV covers it |
| Diff highlighting / side-by-side view | iotools | **out of model here** | the generic page renders one text output; `output = report` gives the before/after pairing as CSV |
| PWA / offline install | monocalc | **out of model** | page shell is generated by this repo's generic generator |

### Where our defaults deliberately differ

Competitors that dedupe default to "keep the first occurrence of each key", which silently
destroys legitimately repeated params (`tag=a&tag=b`, `id=1&id=2`). The default here is `exact`:
only byte-identical `key=value` pairs collapse, so a repeated-key multi-value param survives a
normalization pass unchanged. `first`/`last` remain available for people who want the
competitor behaviour, and the FAQ says which to pick.

`space = percent` is the default because `%20` is the form that is unambiguous in both a query
string and a path, and because a canonicalizer's job is to make two spellings of the same URL
converge on one.

## UX notes carried over (behaviour, not copy)

- Bulk textarea, one URL per line, with a multi-line placeholder showing a messy real example.
- Preset chips for the common jobs (canonical cache key, drop tracking, keep an allowlist,
  before/after report) — competitors ship "profiles"; chips are this repo's equivalent.
- The result is plain text with the generated download link the page shell already provides.
- FAQ answers the four questions every competitor's FAQ answers: does reordering params change
  the response, how are repeated keys handled, what happens to encoding, and is anything uploaded.

## Verification

Commands run for this block are listed in the build log for the commit; the full matrix is
`cargo test --workspace`, `scripts/build-block-wasm.sh`, `wasm-pack build`, `cargo install --path
cli`, `sync-tool-manifest.py`, the generator render, a verbatim CLI example check, the Playwright
page spec, and `check-tool-hygiene.py`.
