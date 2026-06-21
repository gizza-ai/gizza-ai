import { test, expect } from './fixtures';

// /tools/rc4-cipher/ does RC4 encrypt/decrypt in-browser (pure wasm). data is a
// multiline <textarea>; operation/key_format/format are <select>; key is a field;
// drop is a numeric field.

test('rc4-cipher page round-trips encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/rc4-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', 's3cr3t');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-drop', '0');
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

test('rc4-cipher page matches the classic Key/Plaintext vector', async ({ page }) => {
  await page.goto('/tools/rc4-cipher/');
  await page.fill('#in-data', 'Plaintext');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', 'Key');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-drop', '0');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('bbf316e8d940af0ad3', { timeout: 15000 });
});

test('rc4-cipher page rejects an empty key', async ({ page }) => {
  await page.goto('/tools/rc4-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', '');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-drop', '0');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('key', { timeout: 15000 });
});
