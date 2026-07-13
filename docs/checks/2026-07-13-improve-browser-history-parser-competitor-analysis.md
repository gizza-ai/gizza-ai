# browser-history-parser — competitor analysis (2026-07-13)

Tool function: take an uploaded browser **history database** (Chrome/Edge `History`, or
Firefox `places.sqlite`) and produce a **unified, searchable visit timeline** — one row per
visit with a human-readable timestamp, URL, page title, visit type, and visit count. Chat + CLI
surfaces only (binary SQLite input, like `sqlite-table-to-csv`/`xlsx-to-csv` → **no standalone
page**).

## Scan (paraphrased — no copy/branding reused)

- **NirSoft BrowserHistoryView** — desktop utility that reads the history DBs of several
  browsers and shows a combined table: URL, Title, Visit Time, Visit Count, Visited From,
  Web Browser. Emphasis on a single merged timeline across browsers/users, sortable, with
  CSV/HTML export. Table-stakes: readable timestamp, title, visit count, browser column.
- **NirSoft MZHistoryView** — Firefox-specific `places.sqlite` viewer: URL, Title, Visit
  Count, Typed Count, First/Last Visit Date, Referrer. Confirms the Firefox `moz_places` +
  `moz_historyvisits` schema and PRTime (µs since 1970) timestamps.
- **Sherlock Forensics Browser Viewer** ($29) — read-only extraction across ~8 browsers with
  **CSV export**; markets "read-only / non-destructive" access. Table-stakes: CSV output,
  read-only parsing, multi-browser auto-detect.
- **SysTools / Foxton Browser History Examiner** — commercial forensic viewers that open
  `places.sqlite`/Chrome `History` into a readable timeline (date, URL, title), with
  search/filter and date sorting.
- **Reference SQL (Wikiversity/forensics guides)** — the canonical Firefox join is
  `SELECT datetime(moz_historyvisits.visit_date/1000000,'unixepoch'), moz_places.url,
  moz_places.title FROM moz_places, moz_historyvisits WHERE moz_places.id =
  moz_historyvisits.place_id`. Chrome equivalent joins `visits.url = urls.id` and converts the
  WebKit timestamp (µs since 1601-01-01) via `visit_time/1e6 - 11644473600`.

## Table-stakes → in-model / out-of-model

| Capability | Decision |
|---|---|
| Auto-detect Chrome/Edge vs Firefox schema | **in-model** — detect by table presence (`urls`+`visits` ⇒ Chromium; `moz_places`+`moz_historyvisits` ⇒ Firefox). |
| Readable UTC timestamp per visit | **in-model** — convert WebKit µs (1601 epoch) and PRTime µs (1970 epoch); hand-rolled civil-date formatter (no chrono). |
| URL + page title per visit | **in-model** — joined from the places/urls table. |
| Visit type / transition (link, typed, bookmark, reload, …) | **in-model** — decode Chrome `transition` core byte and Firefox `visit_type`. |
| Visit count per URL | **in-model** — from `visit_count`. |
| Search / substring filter (URL or title) | **in-model** — `search` param, case-insensitive. |
| Newest-first / oldest-first ordering | **in-model** — `order` param. |
| Row cap | **in-model** — `limit` param (0 = all). |
| CSV **and** JSON export | **in-model** — `format` param (json default, csv for spreadsheets). Browser column included in CSV so exports from several files merge. |
| Read-only / non-destructive | **in-model** — pure parser, never writes; reuses the proven `sqlite-table-to-csv` on-disk b-tree reader (no SQL engine, no libsqlite3). |
| Multi-browser (Safari, IE/Edge-legacy, Opera, downloads DB) | **out-of-model** — Safari `History.db` uses a different schema and Mac epoch; not built. Chromium (Chrome/Edge/Brave/Vivaldi) + Firefox cover the vast majority; documented as the supported set. |
| Merge multiple history files in one call | **out-of-model** — the block model resolves a single file per call (single `url`/`ref`); a spreadsheet merge of several CSV exports is the workaround (browser column supports it). |
| Bookmarks / passwords / cache extraction | **out-of-model** — different databases/scope; not this tool. |

## Not-a-duplicate note

`blocks/sqlite-table-to-csv` is a **generic** single-table CSV dumper (pick a table → raw CSV).
`browser-history-parser` is browser-specific: it auto-detects the browser, **joins** the
url/visit tables, converts the two browser timestamp epochs to readable UTC, decodes visit
types, and emits a sorted/filterable timeline. It **reuses** that block's on-disk SQLite reader
(new public `read_table`) rather than reimplementing SQLite parsing — DRY, one parser.
