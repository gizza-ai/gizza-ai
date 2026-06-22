import { test, expect } from './fixtures';

// RFC 4231 test case 2: key = "Jefe", data = "what do ya want for nothing?".
const MSG = 'what do ya want for nothing?';
const KEY = 'Jefe';
const SHA256 =
  '5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843';

test('hmac-generate page computes HMAC (default sha256 hex)', async ({
  page,
}) => {
  await page.goto('/tools/hmac-generate/');
  await page.fill('#in-message', MSG);
  await page.fill('#in-key', KEY);
  await expect(page.locator('#tool-output')).toHaveText(SHA256, {
    timeout: 15000,
  });
});

test('hmac-generate md5 algorithm', async ({ page }) => {
  await page.goto('/tools/hmac-generate/');
  await page.fill('#in-message', MSG);
  await page.fill('#in-key', KEY);
  await page.selectOption('#in-algorithm', 'md5');
  await expect(page.locator('#tool-output')).toHaveText(
    '750c783e6ab0b503eaa86e310a5db738',
    { timeout: 15000 },
  );
});

test('hmac-generate sha3-512 algorithm', async ({ page }) => {
  await page.goto('/tools/hmac-generate/');
  await page.fill('#in-message', MSG);
  await page.fill('#in-key', KEY);
  await page.selectOption('#in-algorithm', 'sha3-512');
  await expect(page.locator('#tool-output')).toHaveText(
    '5a4bfeab6166427c7a3647b747292b8384537cdb89afb3bf5665e4c5e709350b287baec921fd7ca0ee7a0c31d022a95e1fc92ba9d77df883960275beb4e62024',
    { timeout: 15000 },
  );
});

test('hmac-generate base64 output format', async ({ page }) => {
  await page.goto('/tools/hmac-generate/');
  await page.fill('#in-message', MSG);
  await page.fill('#in-key', KEY);
  await page.selectOption('#in-output_format', 'base64');
  await expect(page.locator('#tool-output')).toHaveText(
    'W9zBRr9gdU5qBCQmCJV1x1oAPwidJzmDnexYuWTsOEM=',
    { timeout: 15000 },
  );
});

test('hmac-generate uppercase checkbox', async ({ page }) => {
  await page.goto('/tools/hmac-generate/');
  await page.fill('#in-message', MSG);
  await page.fill('#in-key', KEY);
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText(SHA256.toUpperCase(), {
    timeout: 15000,
  });
});

test('hmac-generate hex key encoding matches text key', async ({ page }) => {
  await page.goto('/tools/hmac-generate/');
  // "Jefe" as hex is 4a656665 — decoding then keying equals keying "Jefe".
  await page.fill('#in-message', MSG);
  await page.fill('#in-key', '4a656665');
  await page.selectOption('#in-key_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(SHA256, {
    timeout: 15000,
  });
});

test('hmac-generate query-param deep-link prefills and computes', async ({
  page,
}) => {
  await page.goto(
    '/tools/hmac-generate/?message=' +
      encodeURIComponent(MSG) +
      '&key=' +
      KEY +
      '&algorithm=sha512',
  );
  await expect(page.locator('#in-message')).toHaveValue(MSG, {
    timeout: 15000,
  });
  await expect(page.locator('#in-key')).toHaveValue(KEY);
  await expect(page.locator('#in-algorithm')).toHaveValue('sha512');
  await expect(page.locator('#tool-output')).toHaveText(
    '164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737',
    { timeout: 15000 },
  );
});
