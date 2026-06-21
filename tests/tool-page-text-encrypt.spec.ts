import { test, expect } from './fixtures';

// /tools/text-encrypt/ AES-GCM encrypts/decrypts text in-browser (pure wasm).
// text is a multiline <textarea>; mode is a <select>; passphrase a field.
test('text-encrypt page round-trips encrypt then decrypt', async ({ page }) => {
  const secret = 'meet at noon';
  const pass = 'hunter2';

  await page.goto('/tools/text-encrypt/');
  await page.fill('#in-text', secret);
  await page.fill('#in-passphrase', pass);
  await page.selectOption('#in-mode', 'encrypt');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const token = (await out.textContent())!.trim();
  expect(token).not.toContain(secret);
  expect(token.length).toBeGreaterThan(20);

  // Now decrypt the token with the same passphrase.
  await page.fill('#in-text', token);
  await page.fill('#in-passphrase', pass);
  await page.selectOption('#in-mode', 'decrypt');
  await expect(out).toHaveText(secret, { timeout: 15000 });
});

test('text-encrypt page fails clearly on a wrong passphrase', async ({ page }) => {
  await page.goto('/tools/text-encrypt/');
  await page.fill('#in-text', 'hi');
  await page.fill('#in-passphrase', 'a');
  await page.selectOption('#in-mode', 'encrypt');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const token = (await out.textContent())!.trim();

  await page.fill('#in-text', token);
  await page.fill('#in-passphrase', 'b'); // wrong
  await page.selectOption('#in-mode', 'decrypt');
  await expect(out).toContainText('decryption failed', { timeout: 15000 });
});
