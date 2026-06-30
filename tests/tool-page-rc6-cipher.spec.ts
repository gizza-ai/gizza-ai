import { test, expect } from './fixtures';

// /tools/rc6-cipher/ does RC6 encrypt/decrypt in-browser (pure wasm). data is a
// multiline <textarea>; operation/cipher/format are <select>; key/iv are fields.
const KEY_HEX = '0123456789abcdef0123456789abcdef'; // 16-byte key
const IV_HEX = 'fedcba9876543210fedcba9876543210'; // 16-byte CBC IV

test('rc6-cipher page CBC round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/rc6-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', KEY_HEX);
  await page.fill('#in-iv', IV_HEX);
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

test('rc6-cipher page matches an RC6-32/20 ECB vector prefix', async ({ page }) => {
  await page.goto('/tools/rc6-cipher/');
  await page.fill('#in-data', '\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'ecb');
  await page.fill('#in-key', '00000000000000000000000000000000');
  await page.fill('#in-iv', '');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('8fc3a53656b1f778c129df4e9848a41e', { timeout: 15000 });
});

test('rc6-cipher page rejects a missing CBC IV', async ({ page }) => {
  await page.goto('/tools/rc6-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', KEY_HEX);
  await page.fill('#in-iv', '');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('iv', { timeout: 15000 });
});
