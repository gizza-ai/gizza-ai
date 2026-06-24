import { test, expect } from './fixtures';

// /tools/ciphersaber2/ does CipherSaber-2 (RC4 + 10-byte IV + repeated key setup)
// encrypt/decrypt in-browser (pure wasm). data is a multiline <textarea>;
// operation/key_format/format are <select>; key/rounds/iv are fields. Encrypt with
// a fixed iv is deterministic; without an iv it generates a random one.

test('ciphersaber2 page matches a fixed-IV vector then decrypts it', async ({ page }) => {
  await page.goto('/tools/ciphersaber2/');
  await page.fill('#in-data', 'Attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', 'asecretkey');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-rounds', '20');
  await page.fill('#in-iv', '0102030405060708090a');
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  // IV is prepended in the clear; the rest is the RC4 keystream XOR.
  await expect(out).toHaveText('0102030405060708090a937fd7024cb2ceb9efbee3066475', {
    timeout: 15000,
  });

  // Decrypt the ciphertext back to the plaintext (IV read from the bytes).
  await page.fill('#in-data', '0102030405060708090a937fd7024cb2ceb9efbee3066475');
  await page.selectOption('#in-operation', 'decrypt');
  await page.fill('#in-iv', '');
  await expect(out).toHaveText('Attack at dawn', { timeout: 15000 });
});

test('ciphersaber2 page round-trips a random-IV encryption', async ({ page }) => {
  await page.goto('/tools/ciphersaber2/');
  await page.fill('#in-data', 'the quick brown fox');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', 's3cr3t');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-rounds', '20');
  await page.fill('#in-iv', '');
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const ct = (await out.textContent())!.trim();
  expect(ct).toMatch(/^[0-9a-f]+$/);
  expect(ct).not.toContain('quick');

  await page.fill('#in-data', ct);
  await page.selectOption('#in-operation', 'decrypt');
  await expect(out).toHaveText('the quick brown fox', { timeout: 15000 });
});

test('ciphersaber2 page rejects an empty key', async ({ page }) => {
  await page.goto('/tools/ciphersaber2/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', '');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-rounds', '20');
  await page.fill('#in-iv', '');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('key', { timeout: 15000 });
});
