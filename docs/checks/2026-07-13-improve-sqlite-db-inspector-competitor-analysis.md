# sqlite-db-inspector competitor analysis (2026-07-13)

Target tool: inspect a `.sqlite` / `.db` file and report tables, columns, indexes, foreign keys, views, triggers, and row counts without running user queries.

## Competitors scanned

| Tool | Table-stakes seen | UX/control patterns | Fit decision |
|---|---|---|---|
| Gera Tools SQLite Schema Viewer | Schema-only view; tables, columns, indexes, views, triggers; no desktop GUI needed. | File upload/drop, readable grouped schema sections. | In-model: schema catalog, grouped Markdown/JSON report. Out-of-model for this build: standalone browser file picker, because binary document-input tools in this repo are chat/CLI only. |
| Codetap SQLite DB Viewer | Opens SQLite files and lets developers explore/analyze DB contents. | Browser upload, table browsing, schema/data split. | In-model: schema metadata and row counts. Out-of-model: interactive row browser/editor and arbitrary query execution. |
| Init SQLite Data Explorer | Visualizes schema, table structures, database statistics, and supports SQL testing. | Dashboard-style stats, data explorer, SQL console. | In-model: table count, row count, column/index/FK details. Out-of-model: SQL console and live result-grid exploration. |
| ReadOnlySQL | Browser-local read-only SQLite viewer; tables, indexes, views, triggers; can run SELECT queries. | Drag/drop local DB, navigation tree, read-only query UI. | In-model: read-only metadata inspection and privacy-friendly no-server behavior. Out-of-model: arbitrary SELECT execution and interactive tree UI in this no-page binary tool. |
| MojoDocs SQLite Viewer | Opens .db/.sqlite/.sqlite3 in browser; schema inspection, paginated table browsing, SQL highlighting, export. | Upload control, paginated data tables, export buttons. | In-model: schema report and structured JSON output. Out-of-model: paginated table data browsing and export of arbitrary query/table data (covered separately by sqlite-table-to-csv). |

## Built decisions

- Inputs: `url` or uploaded `ref` via the repo's `Input::Document` source pattern.
- Parameters: `format=markdown|json` for human-readable vs structured output; `include_internal=false|true` for sqlite_* catalog noise.
- Report includes table counts, table names, columns, data types, NOT NULL, primary-key markers, defaults, row counts for normal rowid tables, explicit/auto indexes, foreign keys, views, and triggers.
- The implementation reads the SQLite schema catalog and b-trees directly through the existing pure-Rust parser, not libsqlite3 and not user-provided SQL.
- WITHOUT ROWID tables are listed, but row count is explicitly marked unavailable because their index-b-tree layout is not counted by the current parser.

## Not built / out of model

- Arbitrary SQL execution, editing, inserts/updates/deletes, visual ER diagrams, paginated row browsing, and desktop-style navigation trees.
- Standalone browser upload page: current gizza file-input SQLite tools (`sqlite-table-to-csv`, `browser-history-parser`) are chat+CLI only, and this tool follows that pattern.
