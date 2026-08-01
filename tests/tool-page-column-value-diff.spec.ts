import { test, expect } from './fixtures';

const LEFT = `id,name,price
1,Apple,10
2,Banana,20
3,Cherry,30`;

const RIGHT = `id,name,price
1,Apple,12
2,Banana,20
3,Cherry,35`;

test('column-value-diff reports value changes keyed by id, ignoring other columns', async ({ page }) => {
  await page.goto('/tools/column-value-diff/');
  await page.fill('#in-left', LEFT);
  await page.fill('#in-right', RIGHT);
  await page.fill('#in-key', 'id');
  await page.fill('#in-value', 'price');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    'value column "price" · 3 keys matched · 2 changed · 1 unchanged\n~ [1] "10" → "12"\n~ [3] "30" → "35"',
    { timeout: 15000 },
  );
});

test('column-value-diff ignores unrelated columns and different schemas', async ({ page }) => {
  await page.goto('/tools/column-value-diff/');
  // name differs but only price is compared → no differences.
  await page.fill('#in-left', `id,name,price
1,Apple,10`);
  await page.fill('#in-right', `id,name,price
1,Apricot,10`);
  await page.fill('#in-key', 'id');
  await page.fill('#in-value', 'price');

  await expect(page.locator('#tool-output')).toHaveText('No differences.', { timeout: 15000 });
});

test('column-value-diff include-unmatched checkbox and JSON output', async ({ page }) => {
  await page.goto('/tools/column-value-diff/');
  await page.fill('#in-left', `id,qty
1,5
2,7`);
  await page.fill('#in-right', `id,qty
1,6
3,9`);
  await page.fill('#in-key', 'id');
  await page.fill('#in-value', 'qty');
  await page.check('#in-include_unmatched');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"value_column": "qty"', { timeout: 15000 });
  await expect(out).toContainText('"old": "5"');
  await expect(out).toContainText('"new": "6"');
  await expect(out).toContainText('"left_only": 1');
  await expect(out).toContainText('"right_only": 1');
});

test('column-value-diff CSV change-log with semicolon delimiter', async ({ page }) => {
  await page.goto('/tools/column-value-diff/');
  await page.fill('#in-left', `id;price
1;10
2;20`);
  await page.fill('#in-right', `id;price
1;12
3;30`);
  await page.fill('#in-key', 'id');
  await page.fill('#in-value', 'price');
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.check('#in-include_unmatched');
  await page.selectOption('#in-format', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('key,status,old,new', { timeout: 15000 });
  await expect(out).toContainText('1,changed,10,12');
  await expect(out).toContainText('2,left_only,20,');
  await expect(out).toContainText('3,right_only,,30');
});

test('column-value-diff no-header index refs and ignore-case', async ({ page }) => {
  await page.goto('/tools/column-value-diff/');
  await page.fill('#in-left', `1,In Stock
2,Backorder`);
  await page.fill('#in-right', `1,in stock
2,Backorder`);
  await page.fill('#in-key', '1');
  await page.fill('#in-value', '2');
  await page.uncheck('#in-header');
  await page.check('#in-ignore_case');

  // Case-folded, so the only differing-looking value matches → no differences.
  await expect(page.locator('#tool-output')).toHaveText('No differences.', { timeout: 15000 });
});

test('column-value-diff deep link prefills fields and runs', async ({ page }) => {
  const params = new URLSearchParams({
    left: 'id,name,price\n1,Apple,10\n2,Banana,20',
    right: 'id,name,price\n1,Apple,12\n2,Banana,20',
    key: 'id',
    value: 'price',
    format: 'table',
  });
  await page.goto(`/tools/column-value-diff/?${params.toString()}`);

  await expect(page.locator('#in-value')).toHaveValue('price', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    'value column "price" · 2 keys matched · 1 changed · 1 unchanged\n~ [1] "10" → "12"',
    { timeout: 15000 },
  );
});
