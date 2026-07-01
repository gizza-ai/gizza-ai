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

## FAQ

<details>
<summary>Will it tell me if my SQL has a syntax error?</summary>

No — by design. It reformats the token stream without parsing your query
against a grammar, which is what lets it accept any dialect and never reject
unusual syntax. An invalid query comes out nicely formatted but still invalid;
validation is your database's job.

</details>

<details>
<summary>Does it work with PostgreSQL, MySQL, SQL Server…?</summary>

Yes — because it's dialect-agnostic. Instead of implementing one SQL grammar,
it tokenizes and re-lays-out the statement, so PostgreSQL casts, MySQL
backtick identifiers, SQL Server brackets, and SQLite quirks all pass through
unchanged.

</details>

<details>
<summary>Will formatting alter my string literals, identifiers, or comments?</summary>

No. `'string literals'` (including `''` escapes), quoted identifiers, and both
`--` line and `/* … */` block comments are preserved byte-for-byte. Only
recognized SQL keywords are re-cased, and only if you chose `upper` or
`lower` — table and column names are never touched.

</details>

<details>
<summary>Can it minify a query onto one line?</summary>

Not fully — setting **Indent** to 0 removes the indentation, but each major
clause (`SELECT`, `FROM`, `WHERE`, …) still gets its own line. This tool is a
pretty-printer; a dedicated minifier would be the reverse operation.

</details>
