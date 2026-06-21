import { test, expect } from './fixtures';

// /tools/aes-cipher/ does AES encrypt/decrypt in-browser (pure wasm). data is a
// multiline <textarea>; operation/cipher/format are <select>; key/iv are fields.
const KEY = '000102030405060708090a0b0c0d0e0f'; // AES-128, hex
const NONCE = '0f0e0d0c0b0a090807060504'; // 12-byte GCM nonce, hex

test('aes-cipher page GCM round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/aes-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'gcm');
  await page.fill('#in-key', KEY);
  await page.fill('#in-iv', NONCE);
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

test('aes-cipher page rejects a bad key length', async ({ page }) => {
  await page.goto('/tools/aes-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', 'abcd'); // too short
  await page.fill('#in-iv', KEY);
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('key', { timeout: 15000 });
});
