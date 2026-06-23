import { test, expect } from './fixtures';

// /tools/gost-kuznyechik-cipher/ does GOST R 34.12-2015 Kuznyechik encrypt/decrypt
// in-browser (pure wasm). data is a multiline <textarea>; operation/cipher/format
// are <select>; key/iv are fields. Key is a fixed 256-bit (32-byte) value.
const KEY = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f'; // 32-byte, hex
const IV = '0f0e0d0c0b0a09080706050403020100'; // 16-byte IV, hex

test('gost-kuznyechik-cipher page CBC round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/gost-kuznyechik-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', KEY);
  await page.fill('#in-iv', IV);
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const ct = (await out.textContent())!.trim();
  expect(ct).toMatch(/^[0-9a-f]+$/);
  expect(ct).not.toContain('attack');

  await page.fill('#in-data', ct);
  await page.selectOption('#in-operation', 'decrypt');
  await expect(out).toHaveText('attack at dawn', { timeout: 15000 });
});

test('gost-kuznyechik-cipher page rejects a bad key length', async ({ page }) => {
  await page.goto('/tools/gost-kuznyechik-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', 'abcd'); // too short
  await page.fill('#in-iv', IV);
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('key', { timeout: 15000 });
});
