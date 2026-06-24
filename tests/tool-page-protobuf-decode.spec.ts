import { test, expect } from './fixtures';

test('protobuf-decode decodes a hex varint field (default json)', async ({ page }) => {
  await page.goto('/tools/protobuf-decode/');
  // 08 96 01 = message { 1: 150 }. Defaults: encoding=auto, format=json.
  await page.fill('#in-input', '08 96 01');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"field": 1', { timeout: 15000 });
  await expect(out).toContainText('"uint": 150');
  await expect(out).toContainText('"wire_type": "varint"');
});

test('protobuf-decode text format recurses into a nested message', async ({ page }) => {
  await page.goto('/tools/protobuf-decode/');
  // 1a 03 08 96 01 = field 3 { 1: 150 }.
  await page.fill('#in-input', '1a03089601');
  await page.selectOption('#in-format', 'text');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3: message {', { timeout: 15000 });
  await expect(out).toContainText('1: varint 150');
});

test('protobuf-decode decodes base64 input', async ({ page }) => {
  await page.goto('/tools/protobuf-decode/');
  // base64 of 08 96 01.
  await page.fill('#in-input', 'CJYB');
  await page.selectOption('#in-encoding', 'base64');
  await expect(page.locator('#tool-output')).toContainText('"uint": 150', {
    timeout: 15000,
  });
});

test('protobuf-decode query-param deep-link prefills and decodes', async ({ page }) => {
  await page.goto('/tools/protobuf-decode/?input=120774657374696e67&format=text');
  await expect(page.locator('#in-input')).toHaveValue('120774657374696e67', {
    timeout: 15000,
  });
  // field 2 = string "testing".
  await expect(page.locator('#tool-output')).toContainText('2: string "testing"', {
    timeout: 15000,
  });
});
