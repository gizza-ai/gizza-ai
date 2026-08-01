# prisma-schema-from-sql — competitor analysis (2026-07-29)

Function: paste SQL `CREATE TABLE` (and `ALTER TABLE`) DDL → get a Prisma `schema.prisma`
(models, fields, attributes, relations). All findings paraphrased; no competitor copy,
branding, or trademarks reproduced.

## Competitors skimmed (top real tools for this function)

1. **Devzstudio "SQL to Prisma Schema Generator"** (open-source, hosted) — browser-local paste
   box, maps SQL types to Prisma scalar types, handles PK/auto-increment, NOT NULL, UNIQUE,
   DEFAULT, and FK `REFERENCES`. Minimal options; single output pane.
2. **EaseCloud "SQL to Prisma Converter"** — auto-maps SQL data types → Prisma types, handles
   relationships, constraints, and default values. Emphasises "in your browser / private".
3. **SyntaxSnap "SQL to Prisma Converter"** — maps SQL types → Prisma types, nullable fields,
   defaults, primary keys, unique constraints. Marketed as one-click automation.
4. **CodeTidy "SQL to ORM Converter"** (adjacent) — one input → many ORMs (Prisma, Sequelize,
   TypeORM, SQLAlchemy, Django). Prisma is one target among several.

(AnsaTools is a near-identical clone of #1; counted as one design point.)

## Table-stakes params / behaviour observed

| Feature | Competitors | Decision (in/out of model) |
|---|---|---|
| SQL type → Prisma scalar (Int/BigInt/String/Boolean/DateTime/Float/Decimal/Json/Bytes) | all | **in** — core mapping |
| Primary key → `@id`, composite → `@@id` | all | **in** |
| Auto-increment/SERIAL → `@default(autoincrement())` | all | **in** |
| Nullable → `?` | all | **in** |
| `UNIQUE` col → `@unique`, table-level → `@@unique` | all | **in** |
| `DEFAULT` → `@default(...)` incl. `now()`, literals, `uuid()`, `dbgenerated()` | most | **in** |
| Foreign keys → `@relation(fields/references)` + back-relation list field | #1/#2 | **in** (toggle `relations`) |
| `datasource` + `generator` header blocks with chosen provider | most | **in** (toggle `header`) |
| Provider select (postgresql/mysql/sqlite/sqlserver) | most | **in** (`provider` enum) |
| Native DB types (`@db.VarChar(n)`, `@db.Char(n)`, `@db.Decimal(p,s)`) | #2 | **in** (toggle `native_types`, skipped for sqlite) |
| Model/field name mapping (PascalCase model, camelCase field + `@map`/`@@map`) | #1 | **in** (toggle `map_names`) |
| Indexes → `@@index` / `@@unique` | #1 | **in** |
| `enum`/`SET` → generated Prisma `enum` blocks | none reliably | **out (rejected v1)** — mapped to `String`, stated as a limit |
| `@updatedAt` inference from `ON UPDATE CURRENT_TIMESTAMP` | none | **out** — parser drops `ON UPDATE`; stated as a limit |
| Live DB introspection / server round-trip | some cloud tools | **out-of-model** — gizza is browser-local, no server/DB connection |
| Multi-ORM output (Sequelize/TypeORM/…) | CodeTidy | **out-of-scope** — this tool targets Prisma only |

## UX controls competitors ship (adopted where in-model)

- Paste-and-go single textarea → mirrored (`multiline` SQL field + example chips).
- Provider dropdown → `provider` enum `<select>`.
- Toggles for header / relations / native types / naming → boolean checkboxes.
- Worked example (users + orders with FK) shown on the page and as a preset chip.

## Worked example used for docs/tests

Input (postgres):
`CREATE TABLE users (id SERIAL PRIMARY KEY, email VARCHAR(255) NOT NULL UNIQUE, created_at TIMESTAMP DEFAULT now());`
→ model `users` with `id Int @id @default(autoincrement())`, `email String @unique @db.VarChar(255)`,
`created_at DateTime? @default(now())`.
