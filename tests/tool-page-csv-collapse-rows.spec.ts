import { test, expect } from './fixtures';

const CSV = 'region,product\nEast,Apple\nWest,Banana\nEast,Cherry\nEast,Apple';

test('csv-collapse-rows groups rows and dedupes collapsed values', async ({ page }) => {
  await page.goto('/tools/csv-collapse-rows/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-key_columns', 'region');
  await page.fill('#in-collapse_column', 'product');
  await page.check('#in-dedupe');

  await expect(page.locator('#tool-output')).toHaveText('region,product\nEast,"Apple, Cherry"\nWest,Banana', {
    timeout: 15000,
  });
});

test('csv-collapse-rows supports semicolon delimiter, sorting, and blank preservation', async ({ page }) => {
  await page.goto('/tools/csv-collapse-rows/');
  await page.fill('#in-data', 'g;v\nx;b\nx;a\nx;');
  await page.fill('#in-key_columns', 'g');
  await page.fill('#in-collapse_column', 'v');
  await page.fill('#in-separator', '|');
  await page.uncheck('#in-skip_empty');
  await page.selectOption('#in-sort_values', 'asc');
  await page.selectOption('#in-delimiter', 'semicolon');

  await expect(page.locator('#in-skip_empty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('g;v\nx;|a|b', { timeout: 15000 });
});

test('csv-collapse-rows deep-link pre-fills tab-delimited headerless input and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'A\tone\nA\ttwo\nB\tthree',
    key_columns: '1',
    collapse_column: '2',
    separator: '; ',
    dedupe: 'false',
    skip_empty: 'true',
    sort_values: 'none',
    delimiter: 'tab',
    has_header: 'false',
  });

  await page.goto(`/tools/csv-collapse-rows/?${params.toString()}`);
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  await expect(page.locator('#in-has_header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('col_1\tcol_2\nA\tone; two\nB\tthree', {
    timeout: 15000,
  });
});
