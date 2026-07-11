import { test, expect } from './fixtures';

// /tools/json-redact/ masks secrets in JSON in-browser (pure wasm). The JSON
// field is a multiline <textarea>; style is a <select>; detect_values is a
// default-on checkbox.
async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

test('json-redact masks sensitive keys with exact output', async ({ page }) => {
  await page.goto('/tools/json-redact/');
  await page.fill('#in-json', '{"user":"ada","password":"hunter2","api_key":"abc123"}');
  await page.selectOption('#in-style', 'mask');
  await expect(page.locator('#tool-output')).toContainText('"password": "***"', { timeout: 15000 });
  expect(await outText(page)).toBe(`{
  "user": "ada",
  "password": "***",
  "api_key": "***"
}`);
});

test('json-redact preserve-length style and extra key marker', async ({ page }) => {
  await page.goto('/tools/json-redact/');
  await page.fill('#in-json', '{"nickname":"ace","password":"hunter2"}');
  await page.selectOption('#in-style', 'preserve-length');
  await page.fill('#in-extra_keys', 'nickname');
  await expect(page.locator('#tool-output')).toContainText('"password": "*******"', { timeout: 15000 });
  expect(await outText(page)).toBe(`{
  "nickname": "***",
  "password": "*******"
}`);
});

test('json-redact detect_values checkbox off leaves innocuous email value alone', async ({ page }) => {
  await page.goto('/tools/json-redact/');
  await page.fill('#in-json', '{"email":"ada@example.com","note":"ada@example.com"}');
  await page.selectOption('#in-style', 'null');
  await page.uncheck('#in-detect_values');
  await expect(page.locator('#tool-output')).toContainText('"email": null', { timeout: 15000 });
  expect(await outText(page)).toBe(`{
  "email": null,
  "note": "ada@example.com"
}`);
});

test('json-redact deep-link pre-fills and auto-runs', async ({ page }) => {
  const json = encodeURIComponent('{"password":"hunter2","name":"Ada"}');
  await page.goto(`/tools/json-redact/?json=${json}&style=empty`);
  await expect(page.locator('#tool-output')).toContainText('"password": ""', { timeout: 15000 });
  expect(await outText(page)).toBe(`{
  "password": "",
  "name": "Ada"
}`);
});
