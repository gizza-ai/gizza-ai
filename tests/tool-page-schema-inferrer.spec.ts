import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const CSV_INPUT = 'id,email,score,active,joined\n1,a@example.com,9.5,true,2024-01-02\n2,b@example.com,7,false,2024-02-10';

test('schema-inferrer page — emits JSON Schema and Postgres CREATE TABLE', async ({ page }) => {
  await page.goto('/tools/schema-inferrer/');
  await page.fill('#in-data', CSV_INPUT);
  await page.fill('#in-table', 'users');
  await page.fill('#in-primary_key', 'id');
  await expect(page.locator('#tool-output')).toContainText('"title": "users"', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('"score": {\n        "type": "number"');
  expect(out).toContain('"email": {\n        "type": "string",\n        "format": "email"');
  expect(out).toContain('CREATE TABLE "users" (');
  expect(out).toContain('  "id" INTEGER NOT NULL,');
  expect(out).toContain('  PRIMARY KEY ("id")');
});

test('schema-inferrer page — SQL-only MySQL dialect', async ({ page }) => {
  await page.goto('/tools/schema-inferrer/');
  await page.fill('#in-data', 'id,active\n1,true');
  await page.selectOption('#in-output', 'sql');
  await page.selectOption('#in-dialect', 'mysql');
  await page.fill('#in-table', 'users');
  await expect(page.locator('#tool-output')).toContainText('CREATE TABLE `users`', { timeout: 15000 });
  expect(await outputText(page)).toBe('CREATE TABLE `users` (\n  `id` INT NOT NULL,\n  `active` TINYINT(1) NOT NULL\n);');
});

test('schema-inferrer page — JSON Draft-07 omits uuid format', async ({ page }) => {
  await page.goto('/tools/schema-inferrer/');
  await page.fill('#in-data', 'uid\n550e8400-e29b-41d4-a716-446655440000\n550e8400-e29b-41d4-a716-446655440001');
  await page.selectOption('#in-output', 'json-schema');
  await page.selectOption('#in-draft', 'draft-07');
  await page.fill('#in-table', 'sessions');
  await expect(page.locator('#tool-output')).toContainText('draft-07', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('"$schema": "http://json-schema.org/draft-07/schema#"');
  expect(out).toContain('"title": "sessions"');
  expect(out).not.toContain('"format": "uuid"');
});

test('schema-inferrer page — not-null checkbox off leaves full columns nullable in SQL', async ({ page }) => {
  await page.goto('/tools/schema-inferrer/');
  await page.fill('#in-data', 'id\n1\n2');
  await page.selectOption('#in-output', 'sql');
  await page.uncheck('#in-not_null');
  await expect(page.locator('#tool-output')).toContainText('CREATE TABLE "my_table"', { timeout: 15000 });
  expect(await outputText(page)).toBe('CREATE TABLE "my_table" (\n  "id" INTEGER\n);');
});

test('schema-inferrer page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/schema-inferrer/?data=' +
      encodeURIComponent(CSV_INPUT) +
      '&format=csv&delimiter=comma&has_header=true&output=sql&table=users&dialect=postgres&draft=2020-12&primary_key=id&not_null=true&detect_formats=true',
  );
  await expect(page.locator('#in-data')).toHaveValue(CSV_INPUT, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('sql');
  await expect(page.locator('#tool-output')).toContainText('CREATE TABLE "users"', { timeout: 15000 });
  expect(await outputText(page)).toContain('PRIMARY KEY ("id")');
});
