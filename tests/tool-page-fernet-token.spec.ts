import { test, expect } from './fixtures';

// /tools/fernet-token/ creates and reads Fernet tokens (AES-128-CBC + HMAC-SHA256)
// entirely in-browser (pure wasm). text/key are multiline <textarea>s, mode is a
// <select>, ttl is a number field. The encrypt output is the token plus a line
// "Key (save this to decrypt): <key>".
test('fernet-token page round-trips encrypt then decrypt', async ({ page }) => {
  const secret = 'launch codes 1234';

  await page.goto('/tools/fernet-token/');
  await page.fill('#in-text', secret);
  await page.selectOption('#in-mode', 'encrypt');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });

  const encOut = (await out.textContent())!;
  expect(encOut).not.toContain(secret);
  const token = encOut.split('\n')[0].trim();
  const keyMatch = encOut.match(/Key \(save this to decrypt\): (\S+)/);
  expect(keyMatch).not.toBeNull();
  const key = keyMatch![1];
  expect(token.startsWith('gAAAA')).toBeTruthy();

  // Decrypt the token with the generated key.
  await page.fill('#in-text', token);
  await page.fill('#in-key', key);
  await page.selectOption('#in-mode', 'decrypt');
  await expect(out).toContainText(secret, { timeout: 15000 });
  await expect(out).toContainText('Created:', { timeout: 15000 });
});

test('fernet-token page fails clearly on a wrong key', async ({ page }) => {
  await page.goto('/tools/fernet-token/');
  await page.fill('#in-text', 'hello');
  await page.selectOption('#in-mode', 'encrypt');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const token = (await out.textContent())!.split('\n')[0].trim();

  await page.fill('#in-text', token);
  await page.fill('#in-key', 'cw_0x689RpI-jtRR7oE8h_eQsKImvJapLeSbXpwF4e4=');
  await page.selectOption('#in-mode', 'decrypt');
  await expect(out).toContainText('HMAC verification failed', { timeout: 15000 });
});

test('fernet-token page inspects a token structure without the key', async ({ page }) => {
  const token =
    'gAAAAAAdwJ6wAAECAwQFBgcICQoLDA0ODy021cpGVWKZ_eEwCGM4BLLF_5CV9dOPmrhuVUPgJobwOz7JcbmrR64jVmpU4IwqDA==';
  await page.goto('/tools/fernet-token/');
  await page.fill('#in-text', token);
  await page.selectOption('#in-mode', 'inspect');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Version: 0x80', { timeout: 15000 });
  await expect(out).toContainText('1985-10-26', { timeout: 15000 });
  await expect(out).toContainText('IV: 000102030405060708090a0b0c0d0e0f', { timeout: 15000 });

  // With the matching key, the HMAC is reported valid.
  await page.fill('#in-key', 'cw_0x689RpI-jtRR7oE8h_eQsKImvJapLeSbXpwF4e4=');
  await page.selectOption('#in-mode', 'inspect');
  await expect(out).toContainText('valid (key matches)', { timeout: 15000 });
});

test('fernet-token page decrypts a known token via query-param deep link', async ({ page }) => {
  // Spec test vector: key cw_0x..., token decrypts to "hello", created 1985-10-26.
  const key = 'cw_0x689RpI-jtRR7oE8h_eQsKImvJapLeSbXpwF4e4=';
  const token =
    'gAAAAAAdwJ6wAAECAwQFBgcICQoLDA0ODy021cpGVWKZ_eEwCGM4BLLF_5CV9dOPmrhuVUPgJobwOz7JcbmrR64jVmpU4IwqDA==';
  await page.goto(
    `/tools/fernet-token/?text=${encodeURIComponent(token)}&key=${encodeURIComponent(
      key,
    )}&mode=decrypt`,
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('hello', { timeout: 15000 });
  await expect(out).toContainText('1985-10-26', { timeout: 15000 });
});
