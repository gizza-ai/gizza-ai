# json-to-sql-insert — competitor analysis (2026-07-23)

Tool function: turn a pasted JSON object or array of row objects into SQL `INSERT` statements, optionally with `CREATE TABLE` output. Paraphrased notes only — no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **CodeShack JSON to SQL Converter** — online JSON-to-SQL converter. Table-stakes: paste JSON, set a target table, produce insert statements, optionally generate a create-table script, infer basic column types, and support multiple SQL dialects.
2. **JSONLint JSON to SQL** — JSON-to-SQL page focused on insert/create scripts. Table-stakes: object/array input, MySQL/PostgreSQL/SQLite/SQL Server style output, type inference, readable parse errors, and copyable generated SQL.
3. **JSON Utils JSON to SQL Generator** — schema/script generator from JSON. Table-stakes: automatic column discovery, generated `CREATE TABLE`, generated insert rows, database dialect selection, and examples for pasted JSON.
4. **ToolsCraft / Jsonic JSON to SQL tools** — simple browser tools for JSON arrays. Table-stakes: sample-data presets, table-name control, browser-local conversion, copyable output, and support for a single object as well as arrays.

## Table-stakes → in-model / out-of-model decisions

| Capability | Competitors | Decision |
|---|---|---|
| Paste JSON object or JSON array of objects | all | **in** — `json` textarea accepts a bare object as one row or an array as many rows. |
| Target table name | all | **in** — `table` parameter defaults to `my_table` and supports schema-qualified names. |
| Dialect-specific quoting/literals | database-oriented converters | **in** — enum for `mysql`, `postgres`, `sqlite`, `mssql`, and `ansi`; controls identifier quotes, booleans, placeholders, and basic type names. |
| Literal `INSERT` statements | all | **in** — default `values=literal`, escaping strings for the chosen dialect. |
| Parameterized / prepared statement output | developer-oriented SQL generators | **in** — `values=placeholder`, using `?`, `$n`, or `@pn` plus an ordered params comment. |
| Multi-row batch insert | insert generators | **in** — `multi_row=true` default, with a checkbox to emit separate statements when off. |
| `CREATE TABLE` generation with inferred types | create-table converters | **in** — `create_table` checkbox infers integer/float/boolean/text and supports a primary key. |
| Optional `DROP TABLE IF EXISTS` | script generators | **in** — `drop_table` checkbox, useful for reproducible scratch scripts. |
| Null/default handling | database script generators | **in** — enum maps JSON null or missing keys to `NULL`, `DEFAULT`, or empty string. |
| Deterministic column ordering | serious converter tools | **in** — first-seen order by default; `sort_columns` checkbox for alphabetical output. |
| Browser-local/no upload workflow | privacy-oriented converters | **in** — pure Rust + wasm-bindgen, no network calls. |
| Automatic flattening of nested JSON into many columns | some ETL tools | **out** — nested arrays/objects are preserved as compact JSON strings; relational normalization is ambiguous and beyond a one-step pasted-text tool. |
| Streaming/importing very large files | database import tools | **out** — gizza page model is pasted text / CLI args, not a streaming ETL pipeline. |
| Direct database connection or execution | database clients | **out** — this repo's model is deterministic offline generation, not credentialed DB writes. |
| Full DDL modeling (indexes, foreign keys, constraints, lengths) | schema-design tools | **out** — only basic inferred column types and optional primary key fit the current model. |

## UX/control decisions

- Use a multiline JSON textarea with preset examples for common object/array cases.
- Use enum selects for dialect, values mode, and null handling so the page and CLI accept documented values only.
- Use checkboxes for multi-row batching, create table, drop table, identifier quoting, and sorted columns.
- Include a primary-key text box that is validated against discovered columns.
- Show exact SQL text in the standard text output area; examples include MySQL, PostgreSQL with CREATE TABLE, placeholder output, and per-row statements.

## Distinction from existing blocks

This tool is not a generic format converter. Existing JSON/YAML/TOML and CSV blocks transform data formats; this one generates database-oriented SQL text with dialect-specific quoting, placeholders, null policy, optional DDL, and deterministic column ordering from JSON rows.
