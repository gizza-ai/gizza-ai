import { test, expect } from './fixtures';

const CSV = 'id,name,score,active,joined\n1,Alice,9.5,true,2024-01-02\n2,Bob,7,false,2024-02-10';

async function setInput(page: import('@playwright/test').Page, value = CSV) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function sqlOutput(page: import('@playwright/test').Page) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('INSERT INTO', { timeout: 15_000 });
  return out;
}

test('csv-to-sql renders MySQL CREATE TABLE and multi-row INSERT from CSV', async ({ page }) => {
  await page.goto('/tools/csv-to-sql/');
  await setInput(page);
  await page.fill('#in-table', 'users');
  await page.fill('#in-primary_key', 'id');

  const out = await sqlOutput(page);
  await expect(out).toContainText('CREATE TABLE `users`');
  await expect(out).toContainText('`id` INT');
  await expect(out).toContainText('`score` DOUBLE');
  await expect(out).toContainText('`active` TINYINT(1)');
  await expect(out).toContainText('`joined` DATE');
  await expect(out).toContainText('PRIMARY KEY (`id`)');
  await expect(out).toContainText("(1, 'Alice', 9.5, 1, '2024-01-02')");
  await expect(out).toContainText("(2, 'Bob', 7, 0, '2024-02-10')");
});

test('csv-to-sql deep-links Postgres placeholder mode', async ({ page }) => {
  const qs = new URLSearchParams({
    input: 'id,name\n1,Alice\n2,Bob',
    table: 'public.users',
    dialect: 'postgres',
    values: 'placeholder',
    create_table: 'false',
  });
  await page.goto(`/tools/csv-to-sql/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(/id,name/);
  await expect(page.locator('#in-table')).toHaveValue('public.users');
  await expect(page.locator('#in-dialect')).toHaveValue('postgres');
  await expect(page.locator('#in-values')).toHaveValue('placeholder');
  await expect(page.locator('#in-create_table')).not.toBeChecked();

  const out = await sqlOutput(page);
  await expect(out).toContainText('INSERT INTO "public"."users" ("id", "name") VALUES');
  await expect(out).toContainText('($1, $2),');
  await expect(out).toContainText('($3, $4);  -- params: 1, \'Alice\', 2, \'Bob\'');
  await expect(out).not.toContainText('CREATE TABLE');
});

test('csv-to-sql covers delimiters, no-header checkbox, unquoted identifiers, and per-row inserts', async ({ page }) => {
  await page.goto('/tools/csv-to-sql/');
  await setInput(page, '1|Alice\n2|Bob');
  await page.selectOption('#in-delimiter', 'pipe');
  await page.uncheck('#in-has_header');
  await page.uncheck('#in-multi_row');
  await page.uncheck('#in-create_table');
  await page.uncheck('#in-quote_identifiers');
  await page.fill('#in-table', 'users');

  const out = await sqlOutput(page);
  await expect(out).toContainText("INSERT INTO users (column_1, column_2) VALUES (1, 'Alice');");
  await expect(out).toContainText("INSERT INTO users (column_1, column_2) VALUES (2, 'Bob');");
  await expect(out).not.toContainText('CREATE TABLE');
});

test('csv-to-sql covers JSON input, SQLite dialect, null handling, and type flags', async ({ page }) => {
  await page.goto('/tools/csv-to-sql/');
  await setInput(page, '[{"id":1,"zip":"01234","name":null},{"id":2,"zip":"00567"}]');
  await page.selectOption('#in-format', 'json');
  await page.selectOption('#in-dialect', 'sqlite');
  await page.selectOption('#in-null_handling', 'empty-string');
  await page.uncheck('#in-detect_dates');
  await page.fill('#in-table', 'people');

  const out = await sqlOutput(page);
  await expect(out).toContainText('CREATE TABLE "people"');
  await expect(out).toContainText('"id" INTEGER');
  await expect(out).toContainText('"zip" TEXT');
  await expect(out).toContainText("(1, '01234', '')");
  await expect(out).toContainText("(2, '00567', '')");
});

test('csv-to-sql reports validation errors', async ({ page }) => {
  await page.goto('/tools/csv-to-sql/');
  await setInput(page, '{not json}');
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15_000 });

  await setInput(page, 'a\n1');
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-primary_key', 'missing');
  await expect(page.locator('#tool-output')).toContainText('not one of the columns', { timeout: 15_000 });
});
