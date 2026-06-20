import { test, expect } from './fixtures';

test('csv-json-convert page: CSV → JSON with type inference (auto)', async ({ page }) => {
  await page.goto('/tools/csv-json-convert/');
  // Default direction=auto, headers checkbox pre-checked (schema default true).
  await page.fill('#in-data', 'name,age\nAlice,30');
  await expect(page.locator('#tool-output')).toHaveText(
    '[{"name":"Alice","age":30}]',
    { timeout: 15000 },
  );
});

test('csv-json-convert page: JSON → CSV unions keys (auto)', async ({ page }) => {
  await page.goto('/tools/csv-json-convert/');
  await page.fill('#in-data', '[{"a":1,"b":2},{"a":3,"c":4}]');
  // auto detects the leading '[' as JSON → CSV.
  await expect(page.locator('#tool-output')).toHaveText('a,b,c\n1,2,\n3,,4', {
    timeout: 15000,
  });
});

test('csv-json-convert page: flatten checkbox expands nested objects', async ({ page }) => {
  await page.goto('/tools/csv-json-convert/');
  await page.fill('#in-data', '[{"name":"Al","addr":{"city":"NYC"}}]');
  await page.selectOption('#in-direction', 'json-to-csv');
  await page.check('#in-flatten');
  await expect(page.locator('#tool-output')).toHaveText(
    'name,addr.city\nAl,NYC',
    { timeout: 15000 },
  );
});

test('csv-json-convert page: headers off gives arrays-of-arrays', async ({ page }) => {
  await page.goto('/tools/csv-json-convert/');
  await page.fill('#in-data', 'a,b\nc,d');
  await page.selectOption('#in-direction', 'csv-to-json');
  await page.uncheck('#in-headers');
  await expect(page.locator('#tool-output')).toHaveText('[["a","b"],["c","d"]]', {
    timeout: 15000,
  });
});

test('csv-json-convert page: query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/csv-json-convert/?data=' +
      encodeURIComponent('x,y\n1,2') +
      '&direction=csv-to-json',
  );
  await expect(page.locator('#in-data')).toHaveValue('x,y\n1,2', { timeout: 15000 });
  await expect(page.locator('#in-direction')).toHaveValue('csv-to-json');
  await expect(page.locator('#tool-output')).toHaveText('[{"x":1,"y":2}]', {
    timeout: 15000,
  });
});
