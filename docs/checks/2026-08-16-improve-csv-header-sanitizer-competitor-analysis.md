# csv-header-sanitizer competitor analysis (2026-08-16)

## Scope

Tool: take a CSV/TSV whose header row holds human-written labels (`First Name`, `Total ($)`, `2024 Revenue`, a blank cell, two columns both called `Notes`) and rewrite that row into valid, consistent identifiers — snake_case by default — deduplicating collisions so no column silently overwrites another downstream.

Research done 2026-08-16 via web search plus direct reads of the two reference implementations that the online tools are all modelled on. Findings are paraphrased; no competitor copy, naming, or branding is reused.

## Competitor scan

| Source | Table-stakes found | In-model decision |
| --- | --- | --- |
| R `janitor::make_clean_names()` / `clean_names()` (read the documented argument list directly) | snake_case is the DEFAULT output case, with alternative cases selectable; punctuation removed; spaces and dots become underscores; accented characters transliterated to ASCII under a default-on `ascii` toggle; **duplicate names get numeric suffixes `_2`, `_3` by default**, with an `allow_dupes` escape hatch; a syntactic-safety pass enforces valid identifiers. | Built: `style` enum defaulting to `snake`; default-on `ascii` toggle; `dedupe` enum defaulting to numeric suffixes with an `allow` escape hatch; leading-digit repair as an explicit option. |
| Python `pyjanitor.clean_names()` (read the documented parameter table directly) | Parameters: `case_type` (`lower`/`upper`/`preserve`/`snake`), `strip_underscores` (left/right/both), `strip_whitespace` (default on), `remove_special` (default off), `strip_accents` (default on), `truncate_limit` (max formatted name length, default none), and preservation of the original labels for reference. | Built: case styles incl. `preserve` and `upper` (as `screaming_snake`); whitespace/special-character stripping is unconditional (it is the whole point of the tool, not an option); accent stripping as the `ascii` toggle; `truncate_limit` as `max_length` (0 = no limit); original→cleaned label preservation as the `mapping` output mode. Leading/trailing separators are always stripped rather than exposed as a tri-state, which is the "considered, rejected" item below. |
| Browser-local "CSV header standardizer / CSV cleaner" web tools (search sweep: several independent free tools, all claiming in-browser, no-upload processing) | The recurring feature set is: pick a target case from snake_case / camelCase / PascalCase / kebab-case / UPPER_SNAKE_CASE; fix duplicate headers; fill in empty headers; work on a pasted CSV rather than only a header list; run locally with no upload. | Built: all five case styles plus `preserve`; blank headers filled from a configurable `blank_name` base plus the column position; whole-CSV input with the data rows passed through untouched. In-browser/no-upload is already the platform default here. |
| PostgreSQL lexical rules + BigQuery column-name rules (identifier constraints the "SQL-safe header" claim actually rests on) | An unquoted identifier must start with a letter or underscore — a **leading digit is invalid** unless the name is double-quoted; PostgreSQL truncates identifiers at 63 bytes (`NAMEDATALEN` 64); BigQuery allows up to 300 characters and merely discourages leading digits. | Built: `leading_digit` enum (`underscore` prefix by default, `col_` prefix, or `keep`) and `max_length` with 63 called out on the page and offered as a preset chip. The 300-character ceiling is the `max_length` maximum. |

## Parameters and defaults

| Capability | Default / options | Status |
| --- | --- | --- |
| Target identifier case | `style` = `snake` (default), `camel`, `pascal`, `kebab`, `screaming_snake`, `lower`, `preserve` | In model, built as `Param::enumv` → page `<select>` with friendly labels. |
| Collision handling | `dedupe` = `suffix` (default: `total`, `total_2`, `total_3`), `index` (suffix the 1-based column position), `allow` (leave collisions as-is) | In model, built. Default matches both reference implementations. |
| Accent / Unicode transliteration | `ascii` = on by default (`Größe` → `groesse`, `año` → `ano`) | In model, built via `deunicode` (already proven wasm-safe in `blocks/slugify`). |
| Leading-digit repair for SQL identifiers | `leading_digit` = `underscore` (default), `col`, `keep` | In model, built. |
| Length cap | `max_length` = 0 (no limit), max 300; 63 = the PostgreSQL identifier limit | In model, built; truncation happens BEFORE dedupe so suffixed names still fit. |
| Blank / missing header repair | `blank_name` = `column` → `column_1` at position 1 | In model, built. |
| Output shape | `output` = `csv` (default, whole table with the new header), `header` (just the cleaned header row), `mapping` (`original,sanitized` two-column CSV audit trail) | In model, built. Covers pyjanitor's "preserve the original labels" affordance. |
| Delimiter support (CSV/TSV/semicolon/pipe) | `delimiter` = `,` by default, plus `auto`, names, or any single character | In model, built; the separator round-trips unchanged. |
| Strip-underscores as a left/right/both tri-state | — | **Considered, rejected.** Leading/trailing separators are always stripped: for identifier output there is no user who wants `_total_` back, and the tri-state is three extra states to test for a case that only exists because pyjanitor operates on already-mangled pandas labels. |
| SQL reserved-word avoidance (rename `order` → `order_`) | — | **Considered, rejected.** The reserved list differs per dialect (Postgres, MySQL, BigQuery, Snowflake each differ), so any single built-in list would be wrong for most users and would silently rename a legitimate column. The page FAQ documents the workaround (quote the identifier, or set a length cap and rename explicitly). |
| Renaming data VALUES, deduping rows, type inference, encoding/delimiter auto-repair of a whole file | — | Out of model for this tool; covered by the existing `csv-cleaner`, `csv-dedupe`, and `csv-column-type-validator` tools. Not built here. |
| Cloud batch processing, saved rename profiles, accounts/API keys | — | Out of model: this repo's tools are browser-local, no-server, no-account. Not built. |

## UX decisions taken from the scan

- Case style and collision policy are `<select>`s, never free text — every competitor exposes a fixed vocabulary and typing it is fragile.
- Default the whole tool to "snake_case, dedupe with `_2`, transliterate accents" so the first render is already the answer for the majority case, matching both reference implementations' defaults.
- Ship preset example chips for the recurring workflows: the messy-header default pass, a Postgres-safe pass (`max_length = 63`, leading digit prefixed), a camelCase JSON-ish pass, and a mapping-only audit run.
- Show the rename as an auditable `original,sanitized` table (`output = mapping`), because the documented failure mode is a silent one: two source columns cleaning to the same label, one becoming `total_2`, and a later join keyed on `total` quietly dropping it.
- State the limits on the page — the 5,000,000-byte input cap, the 300-character `max_length` ceiling, that only the header row is rewritten, and that truncation runs before deduplication.
