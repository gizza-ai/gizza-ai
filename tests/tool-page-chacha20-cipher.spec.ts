import { test, expect } from './fixtures';

// /tools/chacha20-cipher/ does ChaCha20 / ChaCha20-Poly1305 encrypt/decrypt
// in-browser (pure wasm). data + aad are multiline <textarea>; operation, mode,
// key_format and format are <select>; key, nonce and counter are fields.
// Field order: data, operation, key, nonce, aad, mode, key_format, counter, format.

const KEY = '0123456789abcdef0123456789abcdef'; // 32 bytes
const NONCE = '0123456789ab'; // 12 bytes

test('chacha20-cipher page round-trips a stream-mode encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/chacha20-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', KEY);
  await page.fill('#in-nonce', NONCE);
  await page.selectOption('#in-mode', 'stream');
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

test('chacha20-cipher page produces the expected deterministic stream ciphertext', async ({ page }) => {
  await page.goto('/tools/chacha20-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', KEY);
  await page.fill('#in-nonce', NONCE);
  await page.selectOption('#in-mode', 'stream');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-counter', '0');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('a68781308d5e3d28fbd748f705d1', { timeout: 15000 });
});

test('chacha20-cipher page round-trips an authenticated AEAD encrypt then decrypt', async ({ page }) => {
  await page.goto('/tools/chacha20-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', KEY);
  await page.fill('#in-nonce', NONCE);
  await page.fill('#in-aad', 'header-v1');
  await page.selectOption('#in-mode', 'aead');
  await page.selectOption('#in-key_format', 'text');
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const ct = (await out.textContent())!.trim();
  expect(ct).toMatch(/^[0-9a-f]+$/);

  await page.fill('#in-data', ct);
  await page.selectOption('#in-operation', 'decrypt');
  await expect(out).toHaveText('attack at dawn', { timeout: 15000 });
});

test('chacha20-cipher page rejects a tampered AEAD ciphertext (auth failure)', async ({ page }) => {
  await page.goto('/tools/chacha20-cipher/');
  await page.fill('#in-data', 'attack at dawn');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', KEY);
  await page.fill('#in-nonce', NONCE);
  await page.fill('#in-aad', 'header-v1');
  await page.selectOption('#in-mode', 'aead');
  await page.selectOption('#in-key_format', 'text');
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const ct = (await out.textContent())!.trim();

  // Flip the first hex nibble, then try to decrypt — must fail authentication.
  const tampered = (ct[0] === '0' ? '1' : '0') + ct.slice(1);
  await page.fill('#in-data', tampered);
  await page.selectOption('#in-operation', 'decrypt');
  await expect(out).toContainText('authentication failed', { timeout: 15000 });
});

test('chacha20-cipher page rejects a wrong-length key', async ({ page }) => {
  await page.goto('/tools/chacha20-cipher/');
  await page.fill('#in-data', 'hi');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-key', 'short');
  await page.fill('#in-nonce', NONCE);
  await page.selectOption('#in-mode', 'stream');
  await page.selectOption('#in-key_format', 'text');
  await page.fill('#in-counter', '0');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('32 bytes', { timeout: 15000 });
});
