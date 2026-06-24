import { test, expect } from './fixtures';

// /tools/hkdf-derive/ runs HKDF extract-and-expand in-browser (pure wasm).
// ikm/salt/info/length are fields (ikm/salt/info are multiline textareas);
// mode/ikm_encoding/salt_encoding/info_encoding/hash/encoding are <select>s
// (rendered from Param::enumv).
test('hkdf-derive page derives an RFC 5869 A.1 vector', async ({ page }) => {
  await page.goto('/tools/hkdf-derive/');
  await page.fill('#in-ikm', '0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b');
  await page.selectOption('#in-mode', 'derive');
  await page.selectOption('#in-ikm_encoding', 'hex');
  await page.fill('#in-salt', '000102030405060708090a0b0c');
  await page.selectOption('#in-salt_encoding', 'hex');
  await page.fill('#in-info', 'f0f1f2f3f4f5f6f7f8f9');
  await page.selectOption('#in-info_encoding', 'hex');
  await page.selectOption('#in-hash', 'sha256');
  await page.fill('#in-length', '42');
  await page.selectOption('#in-encoding', 'hex');
  const out = page.locator('#tool-output');
  // RFC 5869 Appendix A.1 OKM.
  await expect(out).toHaveText(
    '3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865',
    { timeout: 15000 }
  );
});

test('hkdf-derive page extract returns the PRK', async ({ page }) => {
  await page.goto('/tools/hkdf-derive/');
  await page.fill('#in-ikm', '0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b');
  await page.selectOption('#in-mode', 'extract');
  await page.selectOption('#in-ikm_encoding', 'hex');
  await page.fill('#in-salt', '000102030405060708090a0b0c');
  await page.selectOption('#in-salt_encoding', 'hex');
  await page.selectOption('#in-hash', 'sha256');
  const out = page.locator('#tool-output');
  // RFC 5869 Appendix A.1 PRK.
  await expect(out).toHaveText(
    '077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5',
    { timeout: 15000 }
  );
});

test('hkdf-derive page deep-links via query params', async ({ page }) => {
  await page.goto(
    '/tools/hkdf-derive/?ikm=0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b&ikm_encoding=hex&hash=sha256&length=42&encoding=hex'
  );
  const out = page.locator('#tool-output');
  // RFC 5869 A.3 — empty salt + empty info, SHA-256, L=42.
  await expect(out).toHaveText(
    '8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8',
    { timeout: 15000 }
  );
});
