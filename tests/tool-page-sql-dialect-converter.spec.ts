import { test, expect } from './fixtures';

test('sql-dialect-converter Postgres -> MySQL rewrites identifiers, autoinc and types', async ({ page }) => {
  await page.goto('/tools/sql-dialect-converter/');
  await page.fill(
    '#in-sql',
    'CREATE TABLE "users" (\n  id SERIAL PRIMARY KEY,\n  email VARCHAR(255),\n  active BOOLEAN\n);',
  );
  await page.selectOption('#in-from', 'postgres');
  await page.selectOption('#in-to', 'mysql');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('CREATE TABLE `users`', { timeout: 15000 });
  await expect(out).toContainText('id INT AUTO_INCREMENT PRIMARY KEY');
  await expect(out).toContainText('email VARCHAR(255)');
  await expect(out).toContainText('active TINYINT(1)');
});

test('sql-dialect-converter MySQL -> Postgres query-param deep-link converts + strips options', async ({ page }) => {
  const sql =
    'CREATE TABLE `orders` (\n  `id` BIGINT AUTO_INCREMENT PRIMARY KEY,\n  `payload` JSON,\n  `created` DATETIME\n) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;';
  await page.goto(
    '/tools/sql-dialect-converter/?sql=' +
      encodeURIComponent(sql) +
      '&from=mysql&to=postgres',
  );
  // Prefill lands the raw SQL in the textarea, and the deep-link auto-computes.
  await expect(page.locator('#in-sql')).toHaveValue(sql, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('CREATE TABLE "orders"', { timeout: 15000 });
  await expect(out).toContainText('"id" BIGSERIAL PRIMARY KEY');
  await expect(out).toContainText('"payload" JSONB');
  // MySQL table-option tail is dropped when the target is not MySQL.
  await expect(out).not.toContainText('ENGINE');
  await expect(out).not.toContainText('CHARSET');
});

test('sql-dialect-converter Postgres -> SQLite deep-link produces INTEGER PRIMARY KEY AUTOINCREMENT', async ({ page }) => {
  const sql =
    'CREATE TABLE events (\n  id SERIAL PRIMARY KEY,\n  ts TIMESTAMP,\n  amount DOUBLE PRECISION\n);';
  await page.goto(
    '/tools/sql-dialect-converter/?sql=' +
      encodeURIComponent(sql) +
      '&from=postgres&to=sqlite',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('id INTEGER PRIMARY KEY AUTOINCREMENT', { timeout: 15000 });
  await expect(out).toContainText('ts TEXT');
  await expect(out).toContainText('amount REAL');
});

test('sql-dialect-converter surfaces the empty-input error when SQL is cleared', async ({ page }) => {
  await page.goto('/tools/sql-dialect-converter/');
  const out = page.locator('#tool-output');
  await page.fill('#in-sql', 'SELECT "a" FROM "t";');
  await page.selectOption('#in-to', 'mysql');
  await expect(out).toContainText('SELECT `a`', { timeout: 15000 });
  // Clearing SQL while the dialect selects still hold values makes convert()
  // reject the empty input, and the page renders that error in the output.
  await page.fill('#in-sql', '');
  await expect(out).toContainText('empty SQL input', { timeout: 15000 });
});
