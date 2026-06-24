import { test, expect } from './fixtures';

// /tools/scrypt-derive/ derives a key from a password in-browser (pure wasm).
// password/salt/n/r/p/length are fields; salt_encoding/encoding/mode are
// <select>s (rendered from Param::enumv).
test('scrypt-derive page derives the RFC 7914 vector 1', async ({ page }) => {
  await page.goto('/tools/scrypt-derive/');
  await page.fill('#in-password', '');
  await page.selectOption('#in-mode', 'derive');
  await page.fill('#in-salt', '');
  await page.selectOption('#in-salt_encoding', 'utf8');
  await page.fill('#in-n', '16');
  await page.fill('#in-r', '1');
  await page.fill('#in-p', '1');
  await page.fill('#in-length', '64');
  await page.selectOption('#in-encoding', 'hex');
  const out = page.locator('#tool-output');
  // RFC 7914 §12 scrypt test vector 1: N=16, r=1, p=1, empty password + salt.
  await expect(out).toHaveText(
    '77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede21442fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906',
    { timeout: 15000 },
  );
});

test('scrypt-derive page verifies a key', async ({ page }) => {
  await page.goto('/tools/scrypt-derive/');
  await page.fill('#in-password', '');
  await page.selectOption('#in-mode', 'verify');
  await page.fill('#in-salt', '');
  await page.selectOption('#in-salt_encoding', 'utf8');
  await page.fill('#in-n', '16');
  await page.fill('#in-r', '1');
  await page.fill('#in-p', '1');
  await page.fill(
    '#in-expected',
    '77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede21442fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('match', { timeout: 15000 });
  await expect(out).not.toContainText('no match');
});

test('scrypt-derive page deep-links via query params', async ({ page }) => {
  await page.goto('/tools/scrypt-derive/?password=&salt=&n=16&r=1&p=1&length=64&encoding=hex');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede21442fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906',
    { timeout: 15000 },
  );
});
