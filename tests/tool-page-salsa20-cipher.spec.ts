import { test, expect } from './fixtures';

// /tools/salsa20-cipher/ does Salsa20 encrypt/decrypt in-browser (pure wasm). data
// is a multiline <textarea>; operation/key_format/format are <select>; key, nonce
// and counter are fields.

test('salsa20-cipher page round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/salsa20-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', '0123456789abcdef');
  await page.fill('#in-nonce', 'deadbeef');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-counter', '0');
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

test('salsa20-cipher page produces a deterministic ciphertext for a fixed key+nonce', async ({ page }) => {
  await page.goto('/tools/salsa20-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', '0123456789abcdef');
  await page.fill('#in-nonce', 'deadbeef');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-counter', '0');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('0d065913366f2bbf510a3551aac8', { timeout: 15000 });
});

test('salsa20-cipher page rejects a wrong-length key', async ({ page }) => {
  await page.goto('/tools/salsa20-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', 'short');
  await page.fill('#in-nonce', 'deadbeef');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-counter', '0');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('16 or 32 bytes', { timeout: 15000 });
});
