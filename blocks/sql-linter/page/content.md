## What this tool does

Paste SQL and get a local, read-only lint report. The tool masks comments and
string literals, then checks the remaining query text for structural syntax
problems plus practical anti-patterns that are visible without connecting to a
database:

- `SYNTAX` — unbalanced parentheses, unterminated strings/comments, and leading
  or trailing commas near SQL clauses.
- `SELECT-STAR` — `SELECT *` or `table.*`, which makes result shapes unstable and
  can fetch more data than intended.
- `IMPLICIT-JOIN` — comma-separated tables in `FROM`, where join predicates are
  buried in `WHERE`.
- `SUBQUERY-NO-ALIAS` — a derived table in `FROM (...)` without a readable alias.
- `BARE-JOIN` — `JOIN` without an explicit type such as `INNER JOIN` or
  `LEFT JOIN`.

It never executes SQL and does not need a database connection.

## Worked example

Input:

```sql
SELECT *
FROM users u, orders o
JOIN payments p ON p.order_id = o.id
WHERE u.id = o.user_id;
```

Default output:

```text
SQL lint (generic) · 3 findings · 0 errors · 2 warnings · 1 info

L1 [warning] SELECT-STAR: avoid SELECT *; list the columns needed so schemas and payloads stay stable
  SELECT *

L2 [warning] IMPLICIT-JOIN: comma-separated tables are an implicit join; use explicit JOIN ... ON clauses
  FROM users u, orders o

L3 [info] BARE-JOIN: bare JOIN leaves the join type implicit; write INNER JOIN, LEFT JOIN, etc.
  JOIN payments p ON p.order_id = o.id
```

Switch **Minimum severity** to **Warnings and errors** to hide info-only style
hints, or **Errors only** when you only want structural syntax problems.

## Limits and edge cases

- This is a heuristic linter, not a full SQLFluff replacement. It catches the
  patterns above reliably for common SQL, but it does not expand dbt/Jinja,
  resolve table schemas, or validate every dialect-specific grammar rule.
- The **Dialect** option is intentionally small. It currently affects comment
  masking (for example MySQL `#` comments) and labels the report; the rules stay
  conservative across dialects.
- `ignore` accepts rule codes such as `SELECT-STAR` or `BARE-JOIN`, separated by
  commas or spaces, when a warning is intentional.
- Use **JSON** output for CI checks or scripts; the JSON includes summary counts
  and a findings array with line, severity, rule code, message, and snippet.

## FAQ

<details>
<summary>Does this run my SQL?</summary>

No. The tool treats SQL as plain text and never connects to a database. It only
parses enough structure to produce a lint report in your browser.

</details>

<details>
<summary>Why does it flag SELECT *?</summary>

`SELECT *` makes result columns change when the table schema changes, can move
more data than needed, and makes code reviews harder. Listing columns makes the
query contract explicit.

</details>

<details>
<summary>Can I use this instead of a full dialect linter?</summary>

Use it for quick local review and portable anti-pattern checks. For a large SQL
codebase with templating, project config, and dialect-specific rules, keep using
a full parser-based linter in CI as well.

</details>

<details>
<summary>How do I suppress a known intentional finding?</summary>

Put the rule code in **Ignore rule codes**, for example `SELECT-STAR` or
`SELECT-STAR, BARE-JOIN`. Suppressed rules are removed after the severity filter.

</details>
