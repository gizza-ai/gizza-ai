## What this tool does

Paste a MongoDB filter document or a `db.collection.find(...)` shell query and this tool converts it into deterministic SQL. It can emit a bare boolean condition, a `WHERE` clause, or a full `SELECT` statement with projection, sort, limit, skip, and count handling.

The parser accepts common Mongo shell syntax: unquoted keys, single quotes, trailing commas, comments, regular-expression literals, `ObjectId()`, `ISODate()`, `new Date()`, numeric helpers such as `NumberLong()`, and MongoDB Extended JSON wrappers like `$oid` and `$date`.

## Worked example

Input:

```javascript
db.orders.find(
  { status: { $in: ["paid", "shipped"] }, total: { $gt: 100 } },
  { _id: 0, orderId: 1, total: 1 }
).sort({ total: -1 }).limit(10).skip(20)
```

With **Output** set to `select`, the ANSI SQL output is:

```sql
SELECT "orderId", "total"
FROM "orders"
WHERE "status" IN ('paid', 'shipped') AND "total" > 100
ORDER BY "total" DESC
LIMIT 10 OFFSET 20;
```

## Options

- **Output**: `where` adds the `WHERE` keyword, `condition` returns only the boolean expression, and `select` builds a complete statement.
- **Dialect**: choose ANSI, PostgreSQL, MySQL/MariaDB, or SQL Server for quoting, regex behavior, JSON extraction, and paging syntax.
- **Table name**: used for `select` output when the input is a bare filter document or when you want to override the MongoDB collection name.
- **Dotted paths**: keep `address.city` as one column name, or translate it as a JSON path extraction for the selected dialect.
- **Quote identifiers**: turn this off when you want unquoted column names.
- **Rename _id to id**: useful for schemas migrated from MongoDB where `_id` became a relational `id` column.

## Supported operators and limits

The translator supports `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$and`, `$or`, `$nor`, field-level `$not`, `$exists`, `$regex`, `$mod`, and `$size` where the SQL dialect has a safe equivalent. Unsupported operators explain why rather than guessing.

Input is limited to **100,000 characters** and nesting is limited to **64 levels**. Aggregation pipelines, writes, `$lookup`, `$group`, `$elemMatch`, `$all`, geo queries, text search, and schema-dependent array rewrites are intentionally not translated because they need collection schema knowledge.

## FAQ

<details>
<summary>Can this convert aggregation pipelines?</summary>

No. A `$match` stage can often be pasted as a normal find filter, but stages such as `$group`, `$lookup`, and `$unwind` require schema and join decisions that are not present in a MongoDB snippet. The tool rejects pipelines instead of inventing a misleading SQL query.

</details>

<details>
<summary>How are dotted fields handled?</summary>

By default `profile.city` is treated as one SQL column name. If your document is stored in a JSON column, set **Dotted paths** to `json`; PostgreSQL uses `->>` with casts, MySQL uses `JSON_UNQUOTE(JSON_EXTRACT(...))`, and SQL Server/ANSI use `JSON_VALUE(...)`.

</details>

<details>
<summary>Does regular expression output work in every SQL dialect?</summary>

PostgreSQL and MySQL have regex operators, so most simple patterns can be emitted there. ANSI SQL and SQL Server do not have a portable regex operator, so only plain anchored patterns can become `LIKE`; complex patterns return an error that suggests switching dialects.

</details>

<details>
<summary>Will the generated SQL be parameterized?</summary>

No. The output is designed to be readable and pasteable. If you use it in application code, replace literal values with your database driver's placeholders and bind parameters before running it against real data.

</details>
