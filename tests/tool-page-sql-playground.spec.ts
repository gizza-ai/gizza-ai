import { test, expect } from './fixtures';

test('sql-playground page runs SQL and formats output', async ({ page }) => {
  await page.goto('/tools/sql-playground/');
  await page.fill(
    '#in-sql',
    "CREATE TABLE users (id INTEGER, name TEXT, age INTEGER); INSERT INTO users VALUES (1,'Ann',30),(2,'Bob',25); SELECT name, age FROM users WHERE age >= 28 ORDER BY age DESC;"
  );
  await page.selectOption('#in-format', 'csv');
  await expect(page.locator('#tool-output')).toHaveText('name,age\nAnn,30', {
    timeout: 20000,
  });
});

test('sql-playground page deep-links via query params', async ({ page }) => {
  const sql = encodeURIComponent('SELECT 6 * 7 AS answer;');
  await page.goto(`/tools/sql-playground/?sql=${sql}&format=csv`);
  await expect(page.locator('#in-sql')).toHaveValue('SELECT 6 * 7 AS answer;', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toHaveText('answer\n42', {
    timeout: 20000,
  });
});
