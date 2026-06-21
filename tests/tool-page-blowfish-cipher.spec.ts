import { test, expect } from './fixtures';

// /tools/blowfish-cipher/ Blowfish encrypt/decrypt in-browser (pure wasm).
// operation/cipher/format are <select>; key/iv/data are fields.
const KEY = '0123456789abcdef'; // Blowfish key, hex (8 bytes)
const IV = 'fedcba9876543210'; // 8-byte IV, hex

test('blowfish-cipher CBC round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/blowfish-cipher/');
  await page.fill('#in-data', 'legacy secret');
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
  await expect(out).toHaveText('legacy secret', { timeout: 15000 });
});
