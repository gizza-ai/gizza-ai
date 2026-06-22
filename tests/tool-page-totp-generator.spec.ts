import { test, expect } from './fixtures';

// /tools/totp-generator/ computes the current TOTP code in-browser (pure wasm,
// time from Date.now()). secret + digits are fields; algorithm is a <select>.
test('totp-generator page produces a current 6-digit code', async ({ page }) => {
  await page.goto('/tools/totp-generator/');
  await page.fill('#in-secret', 'JBSWY3DPEHPK3PXP');
  await page.fill('#in-digits', '6');
  await page.selectOption('#in-algorithm', 'sha1');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('valid for', { timeout: 15000 });
  const txt = (await out.textContent())!.trim();
  expect(txt).toMatch(/^\d{6}\b/);
});

test('totp-generator page errors on a bad secret', async ({ page }) => {
  await page.goto('/tools/totp-generator/');
  await page.fill('#in-secret', '!!! not base32 !!!');
  await page.fill('#in-digits', '6');
  await page.selectOption('#in-algorithm', 'sha1');
  await expect(page.locator('#tool-output')).toContainText('base32', { timeout: 15000 });
});
