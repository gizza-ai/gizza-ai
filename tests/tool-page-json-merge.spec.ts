import { test, expect } from './fixtures';

// /tools/json-merge/ deep-merges JSON in-browser (pure wasm).
test('json-merge deep-merges nested objects (minified)', async ({ page }) => {
  await page.goto('/tools/json-merge/');
  await page.fill('#in-documents', '{"a":1,"nested":{"x":1}}\n{"b":2,"nested":{"y":2}}');
  await page.fill('#in-indent', '0');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('{"a":1,"b":2,"nested":{"x":1,"y":2}}', { timeout: 15000 });
});

test('json-merge concatenates arrays when checked', async ({ page }) => {
  await page.goto('/tools/json-merge/');
  await page.fill('#in-documents', '{"l":[1,2]} {"l":[3]}');
  await page.check('#in-concat_arrays');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('{"l":[1,2,3]}', { timeout: 15000 });
});
