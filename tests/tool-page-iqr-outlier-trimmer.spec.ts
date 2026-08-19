import { test, expect } from './fixtures';

const DATA = `name,price
a,10
b,11
c,12
d,13
e,100`;

const TRIMMED = `name,price
a,10
b,11
c,12
d,13`;

const REPORT = `IQR outlier report — k = 1.5, quartiles = linear, match = any, action = remove

Column: price (column 2)
  numeric values: 5
  Q1: 11
  Q3: 13
  IQR: 2
  lower fence: 8
  upper fence: 16
  out of fence: 1 of 5 (20%)

Rows: 5 total, 1 outlier (20%), 4 clean
Rows in the 'remove' output: 4`;

test('iqr-outlier-trimmer removes the row outside Tukey fences', async ({ page }) => {
  await page.goto('/tools/iqr-outlier-trimmer/');
  await page.fill('#in-data', DATA);
  await page.fill('#in-columns', 'price');
  await page.fill('#in-k', '1.5');
  await page.selectOption('#in-action', 'remove');

  await expect(page.locator('#tool-output')).toHaveText(TRIMMED, { timeout: 15_000 });
});

test('iqr-outlier-trimmer emits an exact quartile/fence report', async ({ page }) => {
  await page.goto('/tools/iqr-outlier-trimmer/');
  await page.fill('#in-data', DATA);
  await page.fill('#in-columns', 'price');
  await page.selectOption('#in-output', 'report');

  await expect(page.locator('#tool-output')).toHaveText(REPORT, { timeout: 15_000 });
});

test('iqr-outlier-trimmer deep-link covers flag action and non-default controls', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'name,price\na,10\nb,11\nc,12\nd,100',
    columns: 'price',
    k: '0',
    action: 'flag',
    output: 'csv',
    header: 'true',
    delimiter: 'comma',
    quartile_method: 'exclusive',
    match_mode: 'all',
    non_numeric: 'remove',
  });
  await page.goto(`/tools/iqr-outlier-trimmer/?${params.toString()}`);

  await expect(page.locator('#in-columns')).toHaveValue('price', { timeout: 15_000 });
  await expect(page.locator('#in-k')).toHaveValue('0');
  await expect(page.locator('#in-action')).toHaveValue('flag');
  await expect(page.locator('#in-quartile_method')).toHaveValue('exclusive');
  await expect(page.locator('#in-match_mode')).toHaveValue('all');
  await expect(page.locator('#in-non_numeric')).toHaveValue('remove');
  await expect(page.locator('#tool-output')).toContainText('name,price,outlier', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('d,100,true');
});

test('iqr-outlier-trimmer covers keep, clip, tab delimiter, and header toggle', async ({ page }) => {
  await page.goto('/tools/iqr-outlier-trimmer/');
  await page.fill('#in-data', '1\t10\n2\t11\n3\t12\n4\t13\n5\t100');
  await page.fill('#in-columns', '2');
  await page.uncheck('#in-header');
  await page.selectOption('#in-delimiter', 'tab');
  await page.selectOption('#in-action', 'keep');
  await expect(page.locator('#tool-output')).toHaveText('5\t100', { timeout: 15_000 });

  await page.selectOption('#in-action', 'clip');
  await expect(page.locator('#tool-output')).toContainText('5\t16', { timeout: 15_000 });
});

test('iqr-outlier-trimmer generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/iqr-outlier-trimmer/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool iqr-outlier-trimmer');
  expect(cli).toContain('name,price');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
