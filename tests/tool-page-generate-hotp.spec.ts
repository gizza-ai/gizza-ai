import { test, expect } from './fixtures';

// /tools/generate-hotp/ computes a counter-based HOTP code in-browser (pure
// wasm, no clock — the counter is supplied). secret/counter/digits are fields;
// algorithm is a <select> (Param::enumv). RFC 4226 vector: secret
// "12345678901234567890" (base32 GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ), counter 0,
// 6 digits, sha1 -> 755224.
test('generate-hotp page produces the RFC 4226 counter-0 code', async ({ page }) => {
  await page.goto('/tools/generate-hotp/');
  await page.fill('#in-secret', 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ');
  await page.fill('#in-counter', '0');
  await page.fill('#in-digits', '6');
  await page.selectOption('#in-algorithm', 'sha1');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('counter 0', { timeout: 15000 });
  const txt = (await out.textContent())!.trim();
  expect(txt).toMatch(/^755224\b/);
});

test('generate-hotp page errors on a bad secret', async ({ page }) => {
  await page.goto('/tools/generate-hotp/');
  await page.fill('#in-secret', '!!! not base32 !!!');
  await page.fill('#in-counter', '0');
  await page.fill('#in-digits', '6');
  await page.selectOption('#in-algorithm', 'sha1');
  await expect(page.locator('#tool-output')).toContainText('base32', { timeout: 15000 });
});
