import { test, expect } from './fixtures';

// /tools/json-yaml-converter/ converts JSON<->YAML in-browser (pure wasm).
test('json-yaml-converter page converts JSON to YAML (auto)', async ({ page }) => {
  await page.goto('/tools/json-yaml-converter/');
  await page.fill('#in-input', '{"name":"Ada","tags":["a","b"]}');
  await expect(page.locator('#tool-output')).toContainText('name: Ada', { timeout: 15000 });
});

test('json-yaml-converter page converts YAML to JSON via deep-link', async ({ page }) => {
  const qs = '?input=' + encodeURIComponent('name: Ada\nage: 30') + '&direction=yaml-to-json';
  await page.goto('/tools/json-yaml-converter/' + qs);
  await expect(page.locator('#tool-output')).toContainText('"name":"Ada"', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"age":30');
});
