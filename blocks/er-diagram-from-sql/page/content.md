## About this tool

ER Diagram from SQL reads the DDL you paste — `CREATE TABLE`, plus `ALTER TABLE ... ADD FOREIGN KEY` and `CREATE UNIQUE INDEX` — and writes the matching Mermaid `erDiagram` source. Each table becomes an entity, each column becomes a `TYPE name` attribute with optional `PK` / `FK` / `UK` markers, and each foreign key becomes one crow's-foot relationship line. No database is contacted and no statement is executed; it is a lenient text parser, so comments and `INSERT` / `SELECT` statements in a dump are simply skipped.

Cardinality is derived from the schema rather than guessed. The parent side is `||` (exactly one) when every foreign-key column is `NOT NULL` and `|o` (zero or one) when any of them is nullable. The child side is `o{` (zero or more), or `o|` when the foreign-key columns are themselves unique in the child table — a primary key, a `UNIQUE` constraint or a unique index — which makes it a one-to-one. The line is solid `--` for a `NOT NULL` foreign key and dashed `..` for a nullable one, because a nullable foreign key means the child can exist without a parent.

### Worked example

This DDL:

```sql
CREATE TABLE users (
  id INT PRIMARY KEY,
  email VARCHAR(255) NOT NULL UNIQUE
);
CREATE TABLE orders (
  id INT PRIMARY KEY,
  user_id INT NOT NULL REFERENCES users(id),
  total DECIMAL(10,2)
);
```

produces this diagram source:

```
erDiagram
    users {
        INT id PK
        VARCHAR(255) email UK
    }
    orders {
        INT id PK
        INT user_id FK
        DECIMAL(10_2) total
    }
    users ||--o{ orders : "user_id"
```

`DECIMAL(10,2)` is rewritten as `DECIMAL(10_2)` on purpose: Mermaid's attribute grammar rejects spaces and commas inside a type token, so types and identifiers are sanitized to safe tokens (`TIMESTAMP WITH TIME ZONE` becomes `TIMESTAMP_WITH_TIME_ZONE`) instead of being dropped. Table names that need it, such as a schema-qualified `public.users`, are double-quoted.

For a wide schema, set **Columns to show** to key columns only or to entities only, drop the relationship labels, and pick an explicit layout direction. Turn on the `*_id` inference when the schema enforces its references in application code instead of with real `FOREIGN KEY` constraints, and turn on the code fence when the destination is a Markdown file or a pull-request comment. Input is capped at 500 tables, because a larger diagram is unreadable in any renderer.

## FAQ

<details>
<summary>Does this render a picture, or just the diagram code?</summary>

It outputs Mermaid `erDiagram` **source code**, not an image. Paste it into anything that renders Mermaid — a GitHub or GitLab Markdown file, issue or comment, Notion, Obsidian, or the Mermaid live editor — and the picture is drawn there. Turn on the code-fence option to get the output already wrapped in a ```` ```mermaid ```` block, ready to paste into Markdown.

</details>

<details>
<summary>What does `users ||--o{ orders` actually mean?</summary>

It is Mermaid's crow's-foot notation: one `users` row relates to zero or more `orders` rows. The marker nearest each entity describes that side — `||` exactly one, `|o` zero or one, `o{` zero or more, `|{` one or more. Here the `orders.user_id` foreign key is `NOT NULL`, so every order has exactly one user, and nothing makes `user_id` unique, so a user can have many orders. Make `user_id` nullable and the line becomes `users |o..o{ orders`; add a `UNIQUE` constraint on it and it becomes `users ||--o| orders`, a one-to-one.

</details>

<details>
<summary>My schema has no FOREIGN KEY constraints — can it still find relationships?</summary>

Yes, with the `*_id` inference turned on. A column such as `company_id` is linked to a table named `company`, `companys`, `companyes` or `companies` when one exists in the same DDL, and the column is then marked `FK` in the diagram. It is a naming heuristic, so it can miss a relationship that does not follow the convention and can occasionally link one you did not mean; explicit `FOREIGN KEY` constraints are always used and are never duplicated by inference. Self-references are skipped.

</details>

<details>
<summary>Why is my many-to-many join table drawn as its own entity?</summary>

Because that is what the schema says, and it is the Mermaid convention. A join table such as `post_tags(post_id, tag_id)` has two foreign keys, so it renders as two one-to-many relationships into a `post_tags` entity rather than as a single many-to-many line between `posts` and `tags`. Collapsing it would hide any payload columns the join table carries, such as `added_at` or a position. If you want the collapsed view, edit the two generated lines into one `posts }o--o{ tags` relationship by hand.

</details>

<details>
<summary>Which SQL is understood, and what is ignored?</summary>

Schema-defining statements: `CREATE TABLE` (inline and table-level `PRIMARY KEY`, `UNIQUE`, `REFERENCES` and `FOREIGN KEY` clauses), `ALTER TABLE ... ADD` forms that add columns or constraints, and `CREATE INDEX` / `CREATE UNIQUE INDEX` — the unique ones matter, because they can turn a one-to-many into a one-to-one. Everything else in a dump — comments, `INSERT`, `SELECT`, `DROP`, stored procedures, triggers, views — is skipped. MySQL/MariaDB, PostgreSQL, SQLite, SQL Server and generic ANSI quoting styles are normalized. Pasting only `INSERT` rows or query results returns an error, since there is no schema to draw.

</details>
