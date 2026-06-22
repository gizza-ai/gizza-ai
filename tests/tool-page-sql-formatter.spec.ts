import { test, expect } from './fixtures';

// /tools/sql-formatter/ pretty-prints SQL in-browser (pure wasm).
test('sql-formatter lays out clauses and uppercases keywords', async ({ page }) => {
  await page.goto('/tools/sql-formatter/');
  await page.fill('#in-sql', 'select id, name from users where age>18 order by name');
  await page.fill('#in-indent', '2');
  await page.selectOption('#in-keyword_case', 'upper');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('SELECT id,', { timeout: 15000 });
  await expect(out).toContainText('  name');
  await expect(out).toContainText('FROM users');
  await expect(out).toContainText('WHERE age > 18');
  await expect(out).toContainText('ORDER BY name');
});

test('sql-formatter respects lower case and indent width', async ({ page }) => {
  await page.goto('/tools/sql-formatter/');
  await page.fill('#in-sql', 'SELECT a, b FROM t');
  await page.fill('#in-indent', '4');
  await page.selectOption('#in-keyword_case', 'lower');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('select a,', { timeout: 15000 });
  await expect(out).toContainText('    b');
  await expect(out).toContainText('from t');
});
