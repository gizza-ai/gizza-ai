# browser-artifact-parser — competitor analysis (2026-07-20)

Tool function: parse an uploaded browser **artifact** SQLite database (history,
cookies, downloads, or cache — Chrome/Chromium, Firefox, or Safari) and merge
every time-stamped record into one unified, searchable forensic timeline.

## Scope decision vs. existing `browser-history-parser`

`blocks/browser-history-parser` already parses a Chrome/Edge or Firefox
**history** DB into a visit timeline, and explicitly refuses cookies, downloads,
cache, and Safari ("different schemas/epochs and are not handled"). This tool is
a deliberate **superset**, not a duplicate: it adds downloads, cookies, and
cache artifact classes, adds Safari (`History.db` + `Cache.db`), and — the core
differentiator — **correlates multiple artifact types from one file into a
single chronological timeline** (a Chromium `History` file yields both visits
*and* downloads, merged). That cross-artifact correlation is the defining
feature of the category leader (Hindsight), which `browser-history-parser` does
not do. Not skiplisted.

## Competitors scanned (top tools for this function)

1. **Hindsight** (open-source Chromium/Firefox browser-forensics tool). The
   category reference. Parses history/URLs, downloads, cache records, cookies,
   bookmarks, autofill, local storage, login-data metadata, extensions, etc.
   Correlates records from each artifact file and places them on one timeline;
   normalizes timestamps to UTC; exports XLSX (default), SQLite, and JSONL.
   Output has a filterable/sortable event-**type** column (URL visited,
   download, …). Firefox: reads `places.sqlite` (history + downloads),
   `cookies.sqlite`, `formhistory.sqlite`.
2. **DB Browser for SQLite** — general visual SQLite editor used as the manual
   workhorse to open the raw tables. Not artifact-aware (no timestamp
   normalization, no timeline correlation); the analyst reads columns by hand.
3. **NirSoft browser-artifact viewers** (a family: a unified cross-browser
   history viewer plus separate cache and cookie viewers). Per-artifact,
   Windows-only GUIs; the unified-history one merges several browsers' history
   but cookies/cache are separate tools rather than one merged timeline.

(Paraphrased from public descriptions; no competitor copy, branding, or
trademarks reproduced.)

## Table-stakes → in-model / out-of-model

| Capability | Competitor | Decision |
| --- | --- | --- |
| Chromium history (urls+visits) → visit timeline | Hindsight, NirSoft | **in** — `urls`+`visits`, WebKit µs epoch |
| Chromium downloads (+ final URL from chain) | Hindsight | **in** — `downloads`+`downloads_url_chains` |
| Chromium cookies (dated by creation) | Hindsight, NirSoft | **in** — `cookies`, WebKit µs epoch |
| Firefox history (places.sqlite) | Hindsight | **in** — `moz_places`+`moz_historyvisits`, PRTime µs |
| Firefox cookies (cookies.sqlite) | Hindsight | **in** — `moz_cookies`, PRTime µs / seconds expiry |
| Firefox downloads | Hindsight | **partial** — legacy `moz_downloads` table parsed; modern `moz_annos`-based downloads listed as a limit (annotation format, not a table) |
| Safari history (History.db) | Hindsight-adjacent | **in** — `history_items`+`history_visits`, CFAbsoluteTime |
| Safari/WebKit cache (Cache.db) | Hindsight, cache viewers | **in** — `cfurl_cache_response`, text UTC timestamp |
| UTC-normalized timestamps | Hindsight | **in** — every source converted to ISO-8601 UTC + unix seconds |
| Unified cross-artifact timeline (correlate types) | Hindsight | **in** — all recognized tables merged + sorted |
| Event **type** column, sortable/filterable | Hindsight | **in** — `kind` column (visit/download/cookie/cache) + `kind` filter param |
| Visit transition/type decode (link/typed/…) | Hindsight | **in** — Chromium `transition` low byte + Firefox `visit_type` decoded |
| Substring search / filter | Hindsight, NirSoft | **in** — `search` param over URL/host/title/info |
| Sort order (newest/oldest) | all | **in** — `order` param |
| Row cap | (usability) | **in** — `limit` param |
| CSV export | Hindsight (CSV/XLSX/JSONL) | **in** — `format=csv`, with a `source` column so multi-file exports merge |
| JSON/JSONL export | Hindsight | **in** — `format=json` structured output |
| XLSX export | Hindsight | **out** — no wasm-safe XLSX *writer* in the toolkit; CSV covers the spreadsheet need |
| Bookmarks / autofill / form history / login-data / local storage | Hindsight | **out of current scope** — separate artifact classes; this tool targets the four time-ordered classes (history, downloads, cookies, cache) that form a chronological timeline. Candidate follow-ups. |
| Cookie/local-storage **value decryption** (OS keychain / DPAPI) | Hindsight | **out** — requires host OS secrets; offline sandbox has no keychain access |
| Chromium/Firefox **disk cache** (index/data files) | Hindsight, cache viewers | **out** — those caches are custom binary formats, not SQLite; only Safari's SQLite `Cache.db` is in scope |

## UX / control patterns matched

- Event-type filter as a fixed-choice control (`kind` enum: all/visit/download/
  cookie/cache) — mirrors Hindsight's filterable Type column.
- Sort order and row cap as first-class params (`order`, `limit`).
- Output format toggle (`format` json/csv) with a merge-friendly `source`
  column — mirrors Hindsight's multi-format, multi-file export.
- No preset chips: this is a no-page block (binary SQLite input has no standalone
  page, like `browser-history-parser` / `sqlite-table-to-csv`), so page-only
  controls (sliders, color pickers, `[[example]]` chips) do not apply. Surfaces
  are chat + CLI only.

## Verification notes

- Correctness of every artifact class + epoch conversion is unit-tested against
  real SQLite fixtures generated by `core/tests/fixtures/gen_fixtures.py`
  (Chrome history+downloads, Chrome cookies, Firefox places, Firefox cookies,
  Safari history, Safari cache, plus a non-browser DB and corrupt-bytes error
  path) — 15 core tests + 10 descriptor/drift tests.
- The network→fetch→parse→envelope pipeline is exercised end-to-end via the CLI
  against public SQLite URLs: the committed Chrome `History` and Firefox
  `places.sqlite` fixtures (success: visits, JSON + CSV, newest/oldest, limit
  truncation, `kind`/`search` filters, invalid-enum + 404 error paths) and a
  real non-browser SQLite (Chinook → clean "not a recognized browser artifact
  database" error). Cookies/downloads/cache/Safari extractors share that exact
  pipeline (only the extractor differs) and are each fixture-tested.
