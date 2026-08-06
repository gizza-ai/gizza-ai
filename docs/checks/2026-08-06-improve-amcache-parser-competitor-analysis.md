# amcache-parser — competitor analysis (2026-08-06)

Scan run **before** implementation so the descriptor shipped with the table stakes already
covered. Competitors were analysed for *ideas and capability coverage only* — no copy, naming,
layout or branding was reproduced.

## Competitors reviewed

| # | Tool | Shape | What it does well |
|---|------|-------|-------------------|
| 1 | Eric Zimmerman's `AmcacheParser` (.NET CLI) | Desktop CLI, Windows | The reference implementation. Walks the modern `Root\Inventory*` containers, decodes every value, and writes one CSV per category (file entries, program entries, drivers, shortcuts, device containers) plus a combined view. Splits file entries into **associated** (their `ProgramId` resolves to a program record) and **unassociated**. Optional whitelist/blacklist hash files. Can replay `.LOG1`/`.LOG2` transaction logs into a dirty hive. |
| 2 | RegRipper `amcache.pl` / `amcache_win8.pl` plugins | Perl plugin over a hive | Handles the **legacy** Win7/Win8 schema (`Root\File\{volume GUID}\{file reference}` with numeric value names, `Root\Programs`). Prints a compact per-entry report with SHA-1 and paths. Timeline-style ordering by key last-write. |
| 3 | Python `AmcacheParser` (ZaikoARG) / `appcompatprocessor.py` | Python CLI | JSON output; folds Amcache into a larger execution-evidence timeline alongside ShimCache. |
| 4 | Velociraptor `Windows.Forensics.Amcache` artifact | Agent artifact | Collects the hive and produces a normalised row set for hunting; the value is the *normalised columns*, not a UI. |
| 5 | Browser/WASM "drop a hive" viewers (e.g. amcacheparser.com) | Client-side web | Local-only parsing (nothing uploaded), free-text filter, export to CSV/JSON. Confirms the browser-local shape is the right one for this family. |

## Feature diff → what shipped

| Capability | Competitors | This tool |
|---|---|---|
| Modern schema (`InventoryApplicationFile`, `InventoryApplication`) | 1, 3, 4, 5 | **Shipped** — `section = files\|programs` |
| Legacy schema (`Root\File`, `Root\Programs`, numeric value names) | 2 (only) | **Shipped** — auto-detected, numeric value names decoded from the documented table |
| Driver binaries (`InventoryDriverBinary`) | 1 | **Shipped** — `section = drivers` |
| Shortcuts (`InventoryApplicationShortcut`) | 1 | **Shipped** — `section = shortcuts` |
| Per-category CSV export | 1, 5 | **Shipped** — `mode = csv`, one header row + one row per entry, category column included |
| Combined timeline | 1, 3 | **Shipped** — `sort = time` (key last-write, newest first) + `mode = bodyfile` for mactime |
| Associated vs unassociated file entries | 1 | **Shipped** — `association = all\|associated\|unassociated`; the program name is resolved and printed next to each file |
| SHA-1 pivot list for VT / IOC matching | 1 (via CSV column) | **Shipped** — `mode = hashes` emits the de-duplicated, prefix-stripped SHA-1 list, ready to paste into the hash-ioc-match tool |
| Free-text filter | 5 | **Shipped** — `filter` matches name, path, publisher, program name and SHA-1, case-insensitively |
| Local-only, no upload | 5 | **Shipped** — pure Rust/wasm, the hive text never leaves the page |
| Entry cap that is *reported*, never silent | — | **Shipped** — `max_entries`, truncation always stated |

### Considered, not built (out of model)

* **Transaction-log (`.LOG1`/`.LOG2`) replay** — needs two extra files alongside the hive; the
  input here is a single pasted blob. A dirty hive still parses, but the un-flushed tail is
  missing; the page says so.
* **Whitelist/blacklist hash files** — a second file input. The `filter` param covers the
  single-hash case, and `hash-ioc-match` already exists for list-vs-list work.
* **Bulk directory processing / server-side batch** — no backend, by design.
* **ShimCache (`AppCompatCache`) correlation** — a different artifact in a different hive
  (SYSTEM); merging them belongs in a dedicated tool, not here.

### Considered, rejected

* **A column-selection param.** Competitors expose ~30 CSV columns. Adding a column picker to a
  chat-facing schema is bloat; `csv` mode emits the full documented column set and users filter
  downstream with the existing CSV tools.
* **JSON output mode.** `csv` covers the machine-readable need and pipes into the rest of the
  toolkit; a third machine format duplicates it.

## Timestamp semantics recorded on the page

Key last-write is the *appraiser's last observation*, not a reliable "first executed" time; PE
link dates are compiler-supplied and attacker-controllable; `InstallDate` is installer-supplied.
All three are surfaced separately and labelled, never merged into one "execution time" column.
