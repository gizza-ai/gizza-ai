import { test, expect } from './fixtures';

const BAD_SQL = [
  'SELECT *',
  'FROM users u, orders o',
  'JOIN payments p ON p.order_id = o.id',
  'WHERE u.id = o.user_id;',
].join('\n');

async function setSql(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-sql').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('sql-linter reports exact text findings for query anti-patterns', async ({ page }) => {
  await page.goto('/tools/sql-linter/');
  await setSql(page, BAD_SQL);

  const expected = [
    'SQL lint (generic) · 3 findings · 0 errors · 2 warnings · 1 info',
    '',
    'L1 [warning] SELECT-STAR: avoid SELECT *; list the columns needed so schemas and payloads stay stable',
    '  SELECT *',
    '',
    'L2 [warning] IMPLICIT-JOIN: comma-separated tables are an implicit join; use explicit JOIN ... ON clauses',
    '  FROM users u, orders o',
    '',
    'L3 [info] BARE-JOIN: bare JOIN leaves the join type implicit; write INNER JOIN, LEFT JOIN, etc.',
    '  JOIN payments p ON p.order_id = o.id',
  ].join('\n');

  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15_000 });
});

test('sql-linter deep-link applies warning filter and ignore list', async ({ page }) => {
  const qs = new URLSearchParams({
    sql: BAD_SQL,
    dialect: 'postgresql',
    min_severity: 'warning',
    ignore: 'SELECT-STAR',
    format: 'text',
  });
  await page.goto(`/tools/sql-linter/?${qs.toString()}`);

  await expect(page.locator('#in-sql')).toHaveValue(BAD_SQL);
  await expect(page.locator('#in-dialect')).toHaveValue('postgresql');
  await expect(page.locator('#in-min_severity')).toHaveValue('warning');
  await expect(page.locator('#in-ignore')).toHaveValue('SELECT-STAR');
  await expect(page.locator('#in-format')).toHaveValue('text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('SQL lint (postgresql) · 1 findings · 0 errors · 1 warnings · 0 info', { timeout: 15_000 });
  await expect(out).toContainText('IMPLICIT-JOIN');
  await expect(out).not.toContainText('SELECT-STAR');
  await expect(out).not.toContainText('BARE-JOIN');
});
