import { test, expect } from './fixtures';

// /tools/gost-magma-cipher/ does GOST 28147-89 / GOST R 34.12-2015 Magma
// encrypt/decrypt in-browser (pure wasm). data is a multiline <textarea>;
// operation/cipher/format are <select>; key/iv are fields. Key is a fixed
// 256-bit (32-byte) value; the block/IV is 64-bit (8 bytes).
const KEY = 'ffeeddccbbaa99887766554433221100f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff'; // 32-byte, hex
const IV = '1234567890abcdef'; // 8-byte IV, hex

test('gost-magma-cipher page CBC round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/gost-magma-cipher/');
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

test('gost-magma-cipher page rejects a bad key length', async ({ page }) => {
  await page.goto('/tools/gost-magma-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', 'abcd'); // too short
  await page.fill('#in-iv', IV);
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('key', { timeout: 15000 });
});
