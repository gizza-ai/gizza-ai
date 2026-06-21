## About this tool

The SQL Formatter pretty-prints and standardizes a SQL query so it's easy to read,
review and diff. Paste a one-line or minified query and it lays the statement out with
each major clause — `SELECT`, `FROM`, `WHERE`, the `JOIN`s, `GROUP BY`, `ORDER BY`,
`HAVING`, `LIMIT` — on its own line, with select-list and column items one per line and
`AND` / `OR` conditions broken onto indented continuation lines.

It is **dialect-agnostic and forgiving**: rather than parsing the query against one
SQL grammar and rejecting anything unusual, it reformats the token stream, so it works on
PostgreSQL, MySQL, SQLite, SQL Server and most other dialects. String literals, quoted
identifiers and `--` / `/* … */` comments are preserved verbatim, and function calls such
as `COUNT(*)` stay tight.

### Options

- **Indent** — spaces of indentation per nesting level, 0 to 8 (default 2).
- **Keyword case** — `upper` (the default, e.g. `SELECT`), `lower` (`select`), or
  `preserve` to keep keywords exactly as you wrote them. Only recognized SQL keywords are
  re-cased; your table and column names are left untouched.

### Privacy

Everything runs locally in your browser via WebAssembly. Your query is never uploaded to
a server.
