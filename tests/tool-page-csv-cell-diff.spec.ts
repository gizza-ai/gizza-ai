import { test, expect } from './fixtures';

const LEFT = `id,name,price
1,Apple,10
2,Banana,20
3,Cherry,30`;

const RIGHT = `id,name,price
1,Apple,12
2,Banana,20
4,Date,15`;

test('csv-cell-diff reports changed, added, and removed keyed rows', async ({ page }) => {
  await page.goto('/tools/csv-cell-diff/');
  await page.fill('#in-left', LEFT);
  await page.fill('#in-right', RIGHT);
  await page.fill('#in-key', 'id');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 columns compared', { timeout: 15000 });
  await expect(out).toContainText('1 rows changed · 1 rows added · 1 rows removed · 1 rows unchanged · 1 cells changed');
  await expect(out).toContainText('~ [1] price: "10" → "12"');
  await expect(out).toContainText('+ [4] id=4, name=Date, price=15');
  await expect(out).toContainText('- [3] id=3, name=Cherry, price=30');
});

test('csv-cell-diff supports JSON output and reordered columns', async ({ page }) => {
  await page.goto('/tools/csv-cell-diff/');
  await page.fill('#in-left', `id,name,price
1,Apple,10`);
  await page.fill('#in-right', `price,id,name
12,1,Apple`);
  await page.fill('#in-key', 'id');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"rows_changed": 1', { timeout: 15000 });
  await expect(out).toContainText('"column": "price"');
  await expect(out).toContainText('"old": "10"');
  await expect(out).toContainText('"new": "12"');
});

test('csv-cell-diff emits CSV change-log and tests a non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/csv-cell-diff/');
  await page.fill('#in-left', `A,10
B,20`);
  await page.fill('#in-right', `A,10
B,21`);
  await page.uncheck('#in-header');
  await page.selectOption('#in-format', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('row_key,status,column,old,new', { timeout: 15000 });
  await expect(out).toContainText('row 2,changed,col2,20,21');
});

test('csv-cell-diff delimiter and ignore-case controls affect comparison', async ({ page }) => {
  await page.goto('/tools/csv-cell-diff/');
  await page.fill('#in-left', `id;name
1;Apple`);
  await page.fill('#in-right', `id;name
1;apple`);
  await page.fill('#in-key', 'id');
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.check('#in-ignore_case');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('No differences.', { timeout: 15000 });
});

test('csv-cell-diff deep link prefills fields and runs', async ({ page }) => {
  const params = new URLSearchParams({
    left: 'id,name,price\n1,Apple,10',
    right: 'id,name,price\n1,Apple,12',
    key: 'id',
    format: 'table',
  });
  await page.goto(`/tools/csv-cell-diff/?${params.toString()}`);

  await expect(page.locator('#in-key')).toHaveValue('id', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('~ [1] price: "10" → "12"', { timeout: 15000 });
});
