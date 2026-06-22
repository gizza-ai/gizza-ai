import { test, expect } from './fixtures';

// /tools/json-yaml-convert/ converts JSON/YAML/TOML in-browser (pure wasm).
test('json to toml', async ({ page }) => {
  await page.goto('/tools/json-yaml-convert/');
  await page.fill('#in-input', '{"title":"x","server":{"port":8080}}');
  await page.selectOption('#in-from', 'json');
  await page.selectOption('#in-to', 'toml');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('title = "x"', { timeout: 15000 });
  await expect(out).toContainText('[server]');
  await expect(out).toContainText('port = 8080');
});

test('yaml to json', async ({ page }) => {
  await page.goto('/tools/json-yaml-convert/');
  await page.fill('#in-input', 'name: gizza\ncount: 3');
  await page.selectOption('#in-from', 'yaml');
  await page.selectOption('#in-to', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "gizza"', { timeout: 15000 });
  await expect(out).toContainText('"count": 3');
});
