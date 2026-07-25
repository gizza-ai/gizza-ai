import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('csv-column-split page — split name column with exact output', async ({ page }) => {
  await page.goto('/tools/csv-column-split/');
  await page.fill('#in-data', 'id,name\n1,"Doe, John"\n2,"Lovelace, Ada"');
  await page.selectOption('#in-mode', 'split');
  await page.fill('#in-columns', 'name');
  await page.fill('#in-separator', ',');
  await page.fill('#in-names', 'last,first');
  await page.fill('#in-max_columns', '0');
  await page.uncheck('#in-keep_source');
  await page.check('#in-trim');
  await page.check('#in-has_header');
  await page.selectOption('#in-delimiter', ',');
  await expect(page.locator('#tool-output')).toContainText('id,last,first', { timeout: 15000 });
  expect(await outputText(page)).toBe('id,last,first\n1,Doe,John\n2,Lovelace,Ada');
});

test('csv-column-split page — concat columns and replace sources', async ({ page }) => {
  await page.goto('/tools/csv-column-split/');
  await page.fill('#in-data', 'first,last,age\nAda,Lovelace,36');
  await page.selectOption('#in-mode', 'concat');
  await page.fill('#in-columns', 'first,last');
  await page.fill('#in-separator', '-');
  await page.fill('#in-names', 'name');
  await page.uncheck('#in-keep_source');
  await page.check('#in-has_header');
  await expect(page.locator('#tool-output')).toContainText('Ada-Lovelace,36', { timeout: 15000 });
  expect(await outputText(page)).toBe('name,age\nAda-Lovelace,36');
});

test('csv-column-split page — deep-link first-only split and keep source', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'loc\n"San Jose, CA, USA"',
    mode: 'split',
    columns: 'loc',
    separator: ',',
    names: 'city,rest',
    max_columns: '2',
    keep_source: 'true',
    trim: 'true',
    has_header: 'true',
    delimiter: ',',
  });
  await page.goto('/tools/csv-column-split/?' + params.toString());
  await expect(page.locator('#in-keep_source')).toBeChecked({ timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('loc,city,rest', { timeout: 15000 });
  expect(await outputText(page)).toBe('loc,city,rest\n"San Jose, CA, USA",San Jose,"CA, USA"');
});
