## About this tool

**SQL Danger Checker** is a static pre-flight review for raw SQL: paste one or more
statements and it flags the ones that can lose or overwrite data **before** you run
them. It never connects to a database and never executes anything — it reads the text
and classifies each statement, so it is safe to run against a query you have not
decided to execute yet.

It flags four kinds of destructive SQL:

- **`DROP DATABASE` / `DROP SCHEMA` / `DROP TABLE` and `TRUNCATE`** — *critical*, they
  permanently delete objects and every row in them.
- **`DROP` of another object (index, view, column …) and `ALTER … DROP …`** — *high*,
  they discard whatever the dropped object held.
- **Other `ALTER`** schema changes — *medium*, review and back up first.
- **`UPDATE` / `DELETE` with no `WHERE`** — or a trivially-true one such as
  `WHERE 1=1` — *high*, because they touch **every** row in the table.

### Worked example

Paste:

```sql
DROP TABLE users;
DELETE FROM sessions;
UPDATE accounts SET balance = 0;
SELECT * FROM orders WHERE id = 42;
```

and you get a report like:

```
3 danger(s) found (1 critical, 2 high, 0 medium, 0 low) in 4 statement(s) scanned

statement 1, line 1  CRITICAL  DROP-TABLE  DROP TABLE permanently removes the table and all of its data.
  DROP TABLE users;

statement 2, line 2  HIGH  DELETE-NO-WHERE  DELETE without a WHERE clause removes ALL rows in the table.
  DELETE FROM sessions;

statement 3, line 3  HIGH  UPDATE-NO-WHERE  UPDATE without a WHERE clause overwrites ALL rows in the table.
  UPDATE accounts SET balance = 0;
```

The `SELECT` is left alone. Each finding gives the statement number, line, severity, a
rule id, and the offending statement so you can jump straight to it. Choose **JSON**
output to pipe the same findings into a script or CI check.

### Options and limits

Set a **dialect** (MySQL additionally treats `#` as a line comment); raise the
**minimum severity** to hide low-risk findings; turn on **strict** to also surface
*guarded* `DELETE`/`UPDATE` (the ones that already have a real `WHERE`) as a low
"confirm before running" note; and use **allow** to suppress categories you know are
intentional (for example `drop, truncate` in a migration).

This is a **heuristic, not a SQL parser**. It splits statements on `;` and masks
comments and string/identifier literals so a `DROP` inside a `--` comment or a
`'string'` is ignored and a `;` inside a string does not split a statement — but it
does not understand stored procedures, dynamic SQL built at runtime, `MERGE`, or a
`WHERE` that is technically present yet still matches every row. Treat findings as
leads to review, not a guarantee, and always test destructive SQL inside a transaction
with a backup.

## FAQ

<details>
<summary>Does this connect to my database or run the SQL?</summary>

No. It is a purely static text scan — it never opens a connection and never executes a
statement. Everything runs locally in your browser via WebAssembly, so the SQL you
paste is never uploaded anywhere. That is exactly why it is safe to check a query you
have not decided to run.

</details>

<details>
<summary>Why is my guarded DELETE/UPDATE not flagged?</summary>

By default a `DELETE` or `UPDATE` that has a real `WHERE` clause is treated as clean,
because it targets specific rows. Turn on **strict** to also list those as a *low*
`…-CONFIRM` finding so every destructive statement gets an explicit look. A `WHERE`
that is always true — for example `WHERE 1=1`, `WHERE 'x'='x'`, or `WHERE true` — is
always flagged as *high*, because it still touches every row.

</details>

<details>
<summary>Can I get machine-readable output for CI?</summary>

Yes. Set **format** to `json` and you get a structured object with a `summary`
(counts of findings by severity and how many statements were scanned) and a `findings`
array, each entry carrying the statement number, line, severity, rule id, category,
message, and a one-line snippet. Fail your pipeline when `summary.critical` (or `high`)
is above zero.

</details>

<details>
<summary>Will it produce false positives or miss things?</summary>

Both are possible — it is a heuristic, not a full parser. It can miss destructive SQL
hidden in dynamic strings, stored procedures, or a `WHERE` clause that is present but
still matches every row, and it may flag a statement you fully intend to run. Use
**allow** to suppress categories you have reviewed (for example `drop` in a schema
migration), and always review the flagged statements yourself before executing.

</details>
