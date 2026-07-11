import { test, expect } from './fixtures';

// #tool-output is set via textContent (raw string), so multi-line output is
// asserted exactly by reading textContent — toHaveText normalizes whitespace
// and cannot compare newlines.
const out = (page: import('@playwright/test').Page) =>
  page.locator('#tool-output');

async function outText(page: import('@playwright/test').Page): Promise<string> {
  return (await out(page).textContent()) ?? '';
}

test('sql-dump-to-csv page — default single table', async ({ page }) => {
  await page.goto('/tools/sql-dump-to-csv/');
  await page.fill('#in-sql', "INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob');");
  await expect
    .poll(() => outText(page), { timeout: 15000 })
    .toBe('id,name\n1,Alice\n2,Bob\n');
});

test('sql-dump-to-csv deep-link (?sql=)', async ({ page }) => {
  const sql = "INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob');";
  await page.goto('/tools/sql-dump-to-csv/?sql=' + encodeURIComponent(sql));
  await expect
    .poll(() => outText(page), { timeout: 15000 })
    .toBe('id,name\n1,Alice\n2,Bob\n');
});

test('sql-dump-to-csv tab delimiter + header off', async ({ page }) => {
  await page.goto('/tools/sql-dump-to-csv/');
  await page.fill('#in-sql', "INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob');");
  await page.selectOption('#in-delimiter', 'tab');
  await page.uncheck('#in-header');
  await expect
    .poll(() => outText(page), { timeout: 15000 })
    .toBe('1\tAlice\n2\tBob\n');
});

test('sql-dump-to-csv quote=all + null_value', async ({ page }) => {
  await page.goto('/tools/sql-dump-to-csv/');
  await page.fill('#in-sql', "INSERT INTO t (a, b) VALUES (NULL, 'x');");
  await page.selectOption('#in-quote', 'all');
  await page.fill('#in-null_value', '\\N');
  await expect
    .poll(() => outText(page), { timeout: 15000 })
    .toBe('"a","b"\n"\\N","x"\n');
});

test('sql-dump-to-csv multiple tables get sections', async ({ page }) => {
  await page.goto('/tools/sql-dump-to-csv/');
  await page.fill(
    '#in-sql',
    'INSERT INTO a (x) VALUES (1);\nINSERT INTO b (y) VALUES (2);',
  );
  await expect
    .poll(() => outText(page), { timeout: 15000 })
    .toBe('### TABLE: a\nx\n1\n\n### TABLE: b\ny\n2\n');
});

test('sql-dump-to-csv BOM prefix', async ({ page }) => {
  await page.goto('/tools/sql-dump-to-csv/');
  await page.fill('#in-sql', 'INSERT INTO t (a) VALUES (1);');
  await page.check('#in-bom');
  await expect
    .poll(() => outText(page), { timeout: 15000 })
    .toBe('﻿a\n1\n');
});
