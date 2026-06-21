import { test, expect } from './fixtures';

// /tools/pbkdf2-derive/ derives a key from a password in-browser (pure wasm).
// password/salt/iterations/length are fields; salt_encoding/hash/encoding are
// <select>s (rendered from Param::enumv).
test('pbkdf2-derive page derives an RFC 6070 vector', async ({ page }) => {
  await page.goto('/tools/pbkdf2-derive/');
  await page.fill('#in-password', 'password');
  await page.selectOption('#in-mode', 'derive');
  await page.fill('#in-salt', 'salt');
  await page.selectOption('#in-salt_encoding', 'utf8');
  await page.fill('#in-iterations', '1');
  await page.selectOption('#in-hash', 'sha1');
  await page.fill('#in-length', '20');
  await page.selectOption('#in-encoding', 'hex');
  const out = page.locator('#tool-output');
  // RFC 6070 PBKDF2-HMAC-SHA1, c=1, dkLen=20.
  await expect(out).toHaveText('0c60c80f961f0e71f3a9b524af6012062fe037a6', { timeout: 15000 });
});

test('pbkdf2-derive page verifies a key', async ({ page }) => {
  await page.goto('/tools/pbkdf2-derive/');
  await page.fill('#in-password', 'password');
  await page.selectOption('#in-mode', 'verify');
  await page.fill('#in-salt', 'salt');
  await page.selectOption('#in-salt_encoding', 'utf8');
  await page.fill('#in-iterations', '1');
  await page.selectOption('#in-hash', 'sha1');
  await page.fill('#in-expected', '0c60c80f961f0e71f3a9b524af6012062fe037a6');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('match', { timeout: 15000 });
  await expect(out).not.toContainText('no match');
});

test('pbkdf2-derive page deep-links via query params', async ({ page }) => {
  await page.goto('/tools/pbkdf2-derive/?password=passwd&salt=salt&iterations=1&hash=sha256&length=16&encoding=base64');
  const out = page.locator('#tool-output');
  // First 16 bytes of the RFC 7914 SHA-256 vector, base64.
  await expect(out).toHaveText('VawEblbjCJ/sFpHCJUS2BQ==', { timeout: 15000 });
});
