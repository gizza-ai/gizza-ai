# sqlite-table-to-csv — competitor analysis (2026-07-10)

Scan performed before implementation/verification. One web search: "online sqlite table to csv converter export sqlite database table csv". Notes below are paraphrased; no competitor wording, branding, example data, or trademarks are copied into the tool.

## Competitors skimmed

1. **RebaseData — SQLite to CSV converter** — upload a SQLite/DB file and receive CSV output; supports multiple SQLite filename extensions and server-side conversion.
2. **BeCSV — Online SQLite viewer/converter** — upload a database, browse tables, run queries, and export to CSV/JSON/Excel-style formats.
3. **SQLiteReader — database converter** — upload SQLite and export database content to CSV/JSON/SQL-like formats.
4. **CSV Shift — SQLite to CSV** — table export plus optional SQL-query export, aimed at quickly turning a database into CSV.
5. **Kanaries — SQLite to CSV** — simple upload-and-convert flow focused on CSV output.

## Table-stakes parameters / features

| Feature | Competitors | In-model here? | Decision |
|---|---|---|---|
| Accept `.db` / `.sqlite` input | all | ✅ | `url` or uploaded `ref` document source |
| List or select tables | all | ✅ | `table` optional; if omitted and ambiguous, error lists tables |
| Export chosen table as CSV | all | ✅ | core parses table b-tree and emits RFC-4180 CSV |
| Header row toggle | common CSV tools | ✅ | `header` boolean defaults true |
| Delimiter choices | converter-style tools | ✅ | `delimiter` enum: comma/tab/semicolon/pipe |
| NULL placeholder | converter/export tools | ✅ | `null_value` string |
| UTF-8 BOM for spreadsheet apps | CSV export tools | ✅ | `bom` boolean |
| Row cap / preview | viewer/export tools | ✅ | `limit` integer, 0 = all |
| Multiple export formats (JSON/XLSX/SQL) | BeCSV, SQLiteReader | ⚠️ out-of-model for this slug | This backlog item is specifically CSV export; other formats belong to separate tools. |
| Arbitrary SQL query execution | BeCSV, CSV Shift | ⚠️ out-of-model | Requires a SQL engine/VM and sandboxing policy; this tool deliberately parses the file format and exports tables only. |
| Edit/browse database UI | viewer tools | ⚠️ out-of-model | Gizza skill/file block returns a conversion artifact, not an interactive database browser. |
| Encrypted/database-password support | some desktop tools | ⚠️ out-of-model | SQLCipher/encrypted pages need crypto/key handling not present in plain SQLite files. |
| WITHOUT ROWID tables | advanced SQLite feature | ⚠️ out-of-model for first pass | Uses a different b-tree layout; the tool errors clearly instead of emitting incorrect CSV. |

## Design decisions

- **Pure file parser, no SQLite engine.** The current gizza model favors deterministic Rust/WASM. The tool reads the SQLite on-disk page format directly, avoiding native `libsqlite3` and arbitrary SQL execution.
- **Table export over query export.** Competitors often include query mode, but table-to-CSV is the core slug. The descriptor exposes `table`, delimiter, header, NULL text, BOM, and row limit — the in-model table-stakes for a converter.
- **Graceful ambiguity.** If a database has multiple user tables and no table is specified, the error lists available tables instead of guessing.
- **Spreadsheet-friendly output.** Header rows default on, CSV quoting follows RFC-4180, optional BOM helps spreadsheet apps, and alternate delimiters cover TSV/semicolon/pipe workflows.
- **Limits are explicit.** The block caps input bytes, truncates LLM-facing text while keeping the full CSV in the UI envelope, and documents unsupported database shapes rather than silently producing partial output.
