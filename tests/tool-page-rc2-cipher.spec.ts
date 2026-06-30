import { test, expect } from './fixtures';

// /tools/rc2-cipher/ does RC2 encrypt/decrypt in-browser (pure wasm). data is a
// multiline <textarea>; operation/cipher/format are <select>; key/iv/effective_key_bits are fields.
const KEY_HEX = '0123456789abcdef'; // 8-byte RC2 key
const IV_HEX = 'fedcba9876543210'; // 8-byte CBC IV

test('rc2-cipher page CBC round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/rc2-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', KEY_HEX);
  await page.fill('#in-iv', IV_HEX);
  await page.fill('#in-effective_key_bits', '64');
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

test('rc2-cipher page matches RFC 2268 ECB vector for a single raw block', async ({ page }) => {
  // The page/API applies PKCS#7 padding, so this verifies the first ciphertext
  // block of an RFC 2268 vector. key=0000000000000000, T1=63, pt=0000000000000000 -> ebb773f993278eff.
  await page.goto('/tools/rc2-cipher/');
  await page.fill('#in-data', '\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'ecb');
  await page.fill('#in-key', '0000000000000000');
  await page.fill('#in-iv', '');
  await page.fill('#in-effective_key_bits', '63');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('ebb773f993278eff', { timeout: 15000 });
});

test('rc2-cipher page rejects a missing CBC IV', async ({ page }) => {
  await page.goto('/tools/rc2-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-cipher', 'cbc');
  await page.fill('#in-key', KEY_HEX);
  await page.fill('#in-iv', '');
  await page.fill('#in-effective_key_bits', '0');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('iv', { timeout: 15000 });
});
