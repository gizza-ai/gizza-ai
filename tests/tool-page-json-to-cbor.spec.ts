import { test, expect } from './fixtures';

// /tools/json-to-cbor/ encodes pasted JSON into CBOR bytes in-browser.
test('json-to-cbor emits canonical hex for an object', async ({ page }) => {
  await page.goto('/tools/json-to-cbor/');
  await page.fill('#in-json', '{"b":2,"a":1}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('a2616101616202', { timeout: 15000 });
});

test('json-to-cbor deep link supports base64 output', async ({ page }) => {
  await page.goto('/tools/json-to-cbor/?json=%5B1%2Ctrue%2Cnull%2C%22x%22%5D&output=base64&canonical=true&group=0');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('hAH19mF4', { timeout: 15000 });
});
