import { test, expect } from './fixtures';

test.beforeEach(async ({ page }) => {
  page.on('pageerror', err => console.log('PAGEERROR', err.stack || err.message));
  page.on('console', msg => console.log('BROWSER', msg.type(), msg.text()));
});

test('json-to-sql-insert page generates exact MySQL multi-row INSERT', async ({ page }) => {
  await page.goto('/tools/json-to-sql-insert/');
  await page.fill('#in-json', '[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]');
  await page.fill('#in-table', 'users');
  await page.selectOption('#in-dialect', 'mysql');
  await page.selectOption('#in-values', 'literal');
  await page.check('#in-multi_row');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INSERT INTO `users`', { timeout: 15000 });
  const expected =
    'INSERT INTO `users` (`id`, `name`) VALUES\n' +
    "(1, 'Alice'),\n" +
    "(2, 'Bob');\n";
  expect(await out.textContent()).toBe(expected);
});

test('json-to-sql-insert deep-link emits Postgres CREATE TABLE and placeholders', async ({ page }) => {
  const qs =
    '?json=' + encodeURIComponent('[{"id":1,"email":"a@x.com","active":true},{"id":2,"email":"b@x.com","active":false}]') +
    '&table=' + encodeURIComponent('accounts') +
    '&dialect=postgres' +
    '&values=placeholder' +
    '&create_table=true' +
    '&primary_key=id' +
    '&multi_row=true';
  await page.goto('/tools/json-to-sql-insert/' + qs);

  await expect(page.locator('#in-dialect')).toHaveValue('postgres', { timeout: 15000 });
  await expect(page.locator('#in-values')).toHaveValue('placeholder');
  await expect(page.locator('#in-create_table')).toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('CREATE TABLE "accounts"', { timeout: 15000 });
  await expect(out).toContainText('PRIMARY KEY ("id")');
  await expect(out).toContainText('INSERT INTO "accounts" ("id", "email", "active") VALUES');
  await expect(out).toContainText('($1, $2, $3),');
  await expect(out).toContainText('($4, $5, $6);  -- params: 1, \'a@x.com\', TRUE, 2, \'b@x.com\', FALSE');
});

test('json-to-sql-insert page supports non-default null and per-row output', async ({ page }) => {
  await page.goto('/tools/json-to-sql-insert/');
  await page.fill('#in-json', '[{"sku":"A1","qty":3},{"sku":"B2"}]');
  await page.fill('#in-table', 'orders');
  await page.selectOption('#in-dialect', 'sqlite');
  await page.selectOption('#in-null_handling', 'default');
  await page.uncheck('#in-multi_row');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INSERT INTO "orders"', { timeout: 15000 });
  const expected =
    'INSERT INTO "orders" ("sku", "qty") VALUES (\'A1\', 3);\n' +
    'INSERT INTO "orders" ("sku", "qty") VALUES (\'B2\', DEFAULT);\n';
  expect(await out.textContent()).toBe(expected);
});
