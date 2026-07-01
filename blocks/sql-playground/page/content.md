## SQL Playground

Write SQL, hit run, see results — no database to install, no server, no signup.
Each run spins up a fresh in-memory database in your browser using WebAssembly,
executes your statements in order, and shows the result of the last one. Nothing
is uploaded; everything happens locally.

### How it works

The database is **empty and brand new on every run**, so include the schema and
data you want to query in the same script:

```sql
CREATE TABLE users (id INTEGER, name TEXT, age INTEGER);
INSERT INTO users VALUES (1, 'Ann', 30), (2, 'Bob', 25), (3, 'Cleo', 41);
SELECT name, age FROM users WHERE age >= 28 ORDER BY age DESC;
```

Separate statements with `;`. The result shown is whatever the **last statement**
produces — a result set for a `SELECT`, or a "rows affected" line for an
`INSERT` / `UPDATE` / `DELETE`.

### What's supported

- **DDL** — `CREATE TABLE`, `DROP TABLE`, `ALTER TABLE`, `CREATE INDEX`.
- **DML** — `INSERT`, `UPDATE`, `DELETE`.
- **Queries** — `SELECT` with `WHERE`, `ORDER BY`, `LIMIT`, `OFFSET`, `GROUP BY`,
  `HAVING`, and `JOIN`.
- **Aggregates & functions** — `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`, plus common
  string/number functions.
- **Types** — `INTEGER`, `FLOAT`, `TEXT`, `BOOLEAN`, `DATE`, `TIMESTAMP`, and
  more, with `NULL` handling.

### Output formats

- **Table** — an aligned ASCII grid, easy to read at a glance.
- **CSV** — comma-separated with a header row, ready to paste into a spreadsheet.
- **JSON** — an array of row objects, handy for piping into other tools.

### Examples

- `SELECT 1 + 1 AS sum;` — quick scratchpad math.
- A `GROUP BY` report:
  `SELECT city, COUNT(*) AS n FROM users GROUP BY city ORDER BY n DESC;`
- A `JOIN` across two tables you create in the same script.

### FAQ

<details>
<summary>Do I need to set up a database?</summary>

No. A fresh in-memory database is created for
each run and discarded afterward — there is nothing to install or connect to.

</details>

<details>
<summary>Is my SQL or data uploaded?</summary>

No. The SQL engine runs locally in your browser
via WebAssembly. Your queries and data never leave your machine.

</details>

<details>
<summary>Is the data saved between runs?</summary>

No. Every run starts from an empty database,
so put your `CREATE TABLE` and `INSERT` statements in the same script as your
query.

</details>

<details>
<summary>Which SQL dialect is this?</summary>

A standards-leaning subset implemented by a
pure-Rust SQL engine — most common `CREATE`/`INSERT`/`SELECT` features work;
engine-specific extensions may not.

</details>
