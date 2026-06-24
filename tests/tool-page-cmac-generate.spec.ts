import { test, expect } from './fixtures';

// NIST SP 800-38B / RFC 4493 AES-128 example key + 16-byte message.
const KEY128 = '2b7e151628aed2a6abf7158809cf4f3c';
const MSG16 = '6bc1bee22e409f96e93d7e117393172a';
const AES128_TAG = '070a16b46b4d4144f79bdd9dd04a287c';
// AES-128-CMAC of the empty message (RFC 4493 example key).
const EMPTY_TAG = 'bb1d6929e95937287fa37d129b756746';

test('cmac-generate page computes AES-128-CMAC (hex key + hex message)', async ({
  page,
}) => {
  await page.goto('/tools/cmac-generate/');
  await page.fill('#in-message', MSG16);
  await page.fill('#in-key', KEY128);
  await page.selectOption('#in-message_encoding', 'hex');
  await page.selectOption('#in-key_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(AES128_TAG, {
    timeout: 15000,
  });
});

test('cmac-generate empty message', async ({ page }) => {
  await page.goto('/tools/cmac-generate/');
  await page.fill('#in-key', KEY128);
  await page.selectOption('#in-key_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(EMPTY_TAG, {
    timeout: 15000,
  });
});

test('cmac-generate base64 output format', async ({ page }) => {
  await page.goto('/tools/cmac-generate/');
  await page.fill('#in-key', KEY128);
  await page.selectOption('#in-key_encoding', 'hex');
  await page.selectOption('#in-output_format', 'base64');
  await expect(page.locator('#tool-output')).toHaveText(
    'ux1pKelZNyh/o30Sm3VnRg==',
    { timeout: 15000 },
  );
});

test('cmac-generate uppercase checkbox', async ({ page }) => {
  await page.goto('/tools/cmac-generate/');
  await page.fill('#in-key', KEY128);
  await page.selectOption('#in-key_encoding', 'hex');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText(
    EMPTY_TAG.toUpperCase(),
    { timeout: 15000 },
  );
});

test('cmac-generate wrong key length shows an error', async ({ page }) => {
  await page.goto('/tools/cmac-generate/');
  await page.fill('#in-message', 'a');
  await page.fill('#in-key', 'short');
  await expect(page.locator('#tool-output.error')).toContainText(
    '16-, 24-, or 32-byte key',
    { timeout: 15000 },
  );
});

test('cmac-generate query-param deep-link prefills and computes', async ({
  page,
}) => {
  await page.goto(
    '/tools/cmac-generate/?message=' +
      MSG16 +
      '&key=' +
      KEY128 +
      '&message_encoding=hex&key_encoding=hex',
  );
  await expect(page.locator('#in-message')).toHaveValue(MSG16, {
    timeout: 15000,
  });
  await expect(page.locator('#in-key')).toHaveValue(KEY128);
  await expect(page.locator('#in-message_encoding')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toHaveText(AES128_TAG, {
    timeout: 15000,
  });
});
