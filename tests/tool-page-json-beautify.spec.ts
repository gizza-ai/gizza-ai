import { test, expect } from './fixtures';

// /tools/json-beautify/ pretty-prints / minifies JSON in-browser (pure wasm).
test('json-beautify pretty-prints with 2-space indent, preserving key order', async ({ page }) => {
  await page.goto('/tools/json-beautify/');
  await page.fill('#in-json', '{"b":1,"a":[1,2]}');
  await page.fill('#in-indent', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"b": 1', { timeout: 15000 });
  await expect(out).toContainText('"a": [');
});

test('json-beautify minifies with indent 0', async ({ page }) => {
  await page.goto('/tools/json-beautify/');
  await page.fill('#in-json', '{\n  "a": 1,\n  "b": 2\n}');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('{"a":1,"b":2}', { timeout: 15000 });
});

test('json-beautify reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/json-beautify/');
  await page.fill('#in-json', '{bad}');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});
