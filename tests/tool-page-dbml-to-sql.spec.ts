import { test, expect } from './fixtures';

const tool = '/tools/dbml-to-sql/';

// Two tables plus a standalone Ref — small enough to assert exactly, but wide
// enough to exercise pk/increment, unique, not null, type mapping and FKs.
const SCHEMA = `Table users {
  id int [pk, increment]
  email varchar [unique, not null]
}
Table orders {
  id int [pk]
  user_id int [not null]
}
Ref: orders.user_id > users.id`;

// Table + column notes and a named unique index, for the comments/indexes flags.
const NOTED_SCHEMA = `Table users {
  id int [pk]
  email varchar [not null, note: "login email"]
  indexes {
    email [unique, name: "idx_users_email"]
  }
  Note: "app users"
}`;

// Argument order MUST match web/src/lib.rs (and meta.toml):
// dbml, dialect, foreign_keys, indexes, comments, if_not_exists,
// drop_if_exists, quote_identifiers.
async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    dbml: SCHEMA,
    dialect: 'auto',
    foreign_keys: 'true',
    indexes: 'true',
    comments: 'true',
    if_not_exists: 'false',
    drop_if_exists: 'false',
    quote_identifiers: 'true',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/dbml-to-sql/gizza_ai_dbml_to_sql_web.js');
    await mod.default('/tools/dbml-to-sql/gizza_ai_dbml_to_sql_web_bg.wasm');
    return mod.run(
      args.dbml,
      args.dialect,
      args.foreign_keys,
      args.indexes,
      args.comments,
      args.if_not_exists,
      args.drop_if_exists,
      args.quote_identifiers,
    );
  }, p);
}

test('dbml-to-sql page compiles a schema into PostgreSQL DDL', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-dbml', SCHEMA);
  await page.selectOption('#in-dialect', 'postgresql');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('CREATE TABLE "users" (', { timeout: 20_000 });
  await expect(output).toContainText('"id" SERIAL PRIMARY KEY');
  await expect(output).toContainText('"email" VARCHAR NOT NULL UNIQUE');
  await expect(output).toContainText('CREATE TABLE "orders" (');
  await expect(output).toContainText('"user_id" INTEGER NOT NULL');
  await expect(output).toContainText(
    'ALTER TABLE "orders" ADD CONSTRAINT "fk_orders_user_id" FOREIGN KEY ("user_id") REFERENCES "users" ("id");',
  );
});

test('dbml-to-sql deep link prefills MySQL plus IF NOT EXISTS and DROP IF EXISTS', async ({ page }) => {
  const qs = new URLSearchParams({
    dbml: SCHEMA,
    dialect: 'mysql',
    foreign_keys: 'true',
    indexes: 'true',
    comments: 'true',
    if_not_exists: 'true',
    drop_if_exists: 'true',
    quote_identifiers: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-dialect')).toHaveValue('mysql', { timeout: 15_000 });
  await expect(page.locator('#in-if_not_exists')).toBeChecked();
  await expect(page.locator('#in-drop_if_exists')).toBeChecked();
  await expect(page.locator('#in-quote_identifiers')).toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('DROP TABLE IF EXISTS `orders`;', { timeout: 20_000 });
  await expect(output).toContainText('DROP TABLE IF EXISTS `users`;');
  await expect(output).toContainText('CREATE TABLE IF NOT EXISTS `users` (');
  await expect(output).toContainText('`id` INT AUTO_INCREMENT PRIMARY KEY');
  await expect(output).toContainText('`email` VARCHAR(255) NOT NULL UNIQUE');
  await expect(output).toContainText(
    'ALTER TABLE `orders` ADD CONSTRAINT `fk_orders_user_id` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`);',
  );
});

test('dbml-to-sql deep link can turn quoting off for SQLite', async ({ page }) => {
  const qs = new URLSearchParams({
    dbml: SCHEMA,
    dialect: 'sqlite',
    foreign_keys: 'false',
    indexes: 'true',
    comments: 'false',
    if_not_exists: 'false',
    drop_if_exists: 'false',
    quote_identifiers: 'false',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-dialect')).toHaveValue('sqlite', { timeout: 15_000 });
  await expect(page.locator('#in-quote_identifiers')).not.toBeChecked();
  await expect(page.locator('#in-foreign_keys')).not.toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('CREATE TABLE users (', { timeout: 20_000 });
  await expect(output).toContainText('id INTEGER PRIMARY KEY AUTOINCREMENT');
  await expect(output).toContainText('email TEXT NOT NULL UNIQUE');
  await expect(output).not.toContainText('FOREIGN KEY');
});

test('dbml-to-sql wasm covers every dialect choice', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-dbml');

  const postgres = await runWasm(page, { dialect: 'postgresql' });
  expect(postgres).toContain('"id" SERIAL PRIMARY KEY');
  expect(postgres).toContain('"user_id" INTEGER NOT NULL');

  const mysql = await runWasm(page, { dialect: 'mysql' });
  expect(mysql).toContain('`id` INT AUTO_INCREMENT PRIMARY KEY');
  expect(mysql).toContain('`email` VARCHAR(255) NOT NULL UNIQUE');

  const sqlite = await runWasm(page, { dialect: 'sqlite' });
  expect(sqlite).toContain('"id" INTEGER PRIMARY KEY AUTOINCREMENT');
  expect(sqlite).toContain('"email" TEXT NOT NULL UNIQUE');

  const sqlserver = await runWasm(page, { dialect: 'sqlserver' });
  expect(sqlserver).toContain('[id] INT IDENTITY(1,1) PRIMARY KEY');
  expect(sqlserver).toContain('[email] NVARCHAR(255) NOT NULL UNIQUE');

  const oracle = await runWasm(page, { dialect: 'oracle' });
  expect(oracle).toContain('"id" NUMBER(10) GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY');
  expect(oracle).toContain('"email" VARCHAR2(255) NOT NULL UNIQUE');

  // auto reads Project { database_type } and falls back to PostgreSQL.
  const auto = await runWasm(page, {
    dbml: "Project app { database_type: 'MySQL' }\nTable notes { id int [pk] body text }",
    dialect: 'auto',
  });
  expect(auto).toContain('CREATE TABLE `notes` (');
  expect(auto).toContain('`body` TEXT');
  expect(await runWasm(page, { dialect: 'auto' })).toContain('CREATE TABLE "users" (');
});

test('dbml-to-sql wasm covers each checkbox and rejects bad input', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-dbml');

  const defaults = await runWasm(page, { dbml: NOTED_SCHEMA, dialect: 'postgresql' });
  expect(defaults).toContain('CREATE UNIQUE INDEX "idx_users_email" ON "users" ("email");');
  expect(defaults).toContain(`COMMENT ON TABLE "users" IS 'app users';`);
  expect(defaults).toContain(`COMMENT ON COLUMN "users"."email" IS 'login email';`);

  const bare = await runWasm(page, {
    dbml: NOTED_SCHEMA,
    dialect: 'postgresql',
    indexes: 'false',
    comments: 'false',
  });
  expect(bare).not.toContain('CREATE UNIQUE INDEX');
  expect(bare).not.toContain('COMMENT ON');

  expect(await runWasm(page, { dialect: 'postgresql', foreign_keys: 'false' })).not.toContain('FOREIGN KEY');
  expect(await runWasm(page, { dialect: 'postgresql', foreign_keys: 'true' })).toContain('FOREIGN KEY');

  const guarded = await runWasm(page, {
    dialect: 'postgresql',
    if_not_exists: 'true',
    drop_if_exists: 'true',
  });
  expect(guarded).toContain('DROP TABLE IF EXISTS "users";');
  expect(guarded).toContain('CREATE TABLE IF NOT EXISTS "users" (');

  const unquoted = await runWasm(page, { dialect: 'postgresql', quote_identifiers: 'false' });
  expect(unquoted).toContain('CREATE TABLE users (');
  expect(unquoted).not.toContain('"users"');

  await expect(runWasm(page, { dbml: '' })).rejects.toThrow(/paste a schema with at least one/);
  await expect(runWasm(page, { dbml: '# just a comment' })).rejects.toThrow(/no `Table` blocks found/);
  await expect(runWasm(page, { dialect: 'duckdb' })).rejects.toThrow(/invalid dialect "duckdb"/);
});

test('dbml-to-sql page shows a usable CLI example', async ({ page }) => {
  await page.goto(tool);
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool dbml-to-sql');
  expect(cli).not.toContain('TODO');
});
