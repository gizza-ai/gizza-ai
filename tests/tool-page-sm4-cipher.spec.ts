import { test, expect } from './fixtures';

// /tools/sm4-cipher/ SM4 encrypt/decrypt in-browser (pure wasm).
// operation/cipher/format are <select>; key/iv/data are fields.
const KEY = '0123456789abcdeffedcba9876543210'; // SM4 key, hex (16 bytes)
const IV = '00112233445566778899aabbccddeeff'; // 16-byte IV, hex

test('sm4-cipher CBC round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/sm4-cipher/');
  await page.fill('#in-data', 'national standard secret');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', KEY);
  await page.fill('#in-iv', IV);
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const ct = (await out.textContent())!.trim();
  expect(ct).toMatch(/^[0-9a-f]+$/);

  await page.fill('#in-data', ct);
  await page.selectOption('#in-operation', 'decrypt');
  await expect(out).toHaveText('national standard secret', { timeout: 15000 });
});
