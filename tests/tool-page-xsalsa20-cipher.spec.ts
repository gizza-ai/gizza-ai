import { test, expect } from './fixtures';

const key = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f';
const nonce = '000102030405060708090a0b0c0d0e0f1011121314151617';
// libsodium crypto_stream_xsalsa20_xor_ic('hello xsalsa20', nonce, ic=0, key)
const expectedHex = '14d30cc3b2bebe35ee3bae0c1603';

test('xsalsa20-cipher page encrypts and decrypts hex', async ({ page }) => {
  await page.goto('/tools/xsalsa20-cipher/');
  await page.fill('#in-data', 'hello xsalsa20');
  await page.fill('#in-key', key);
  await page.fill('#in-nonce', nonce);
  await page.selectOption('#in-operation', 'encrypt');
  await page.selectOption('#in-key_format', 'encoded');
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  await expect(out).toHaveText(expectedHex, { timeout: 15000 });
  const ct = (await out.textContent())!.trim();
  await page.selectOption('#in-operation', 'decrypt');
  await page.fill('#in-data', ct);
  await expect(out).toHaveText('hello xsalsa20', { timeout: 15000 });
});

test('xsalsa20-cipher page validates nonce length', async ({ page }) => {
  await page.goto('/tools/xsalsa20-cipher/');
  await page.fill('#in-data', 'hi');
  await page.fill('#in-key', key);
  await page.fill('#in-nonce', '00');
  await expect(page.locator('#tool-output')).toContainText('24 bytes', { timeout: 15000 });
});

test('xsalsa20-cipher page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/xsalsa20-cipher/?operation=encrypt&data=hello&key=' + key + '&nonce=' + nonce + '&key_format=encoded&counter=0&format=hex');
  await expect(page.locator('#in-data')).toHaveValue('hello');
  // stream cipher: 'hello' encrypts to the first 5 bytes of the 'hello xsalsa20' answer
  await expect(page.locator('#tool-output')).toHaveText(expectedHex.slice(0, 10), { timeout: 15000 });
});

test('xsalsa20-cipher page rejects a malformed counter', async ({ page }) => {
  await page.goto('/tools/xsalsa20-cipher/?operation=encrypt&data=hello&key=' + key + '&nonce=' + nonce + '&key_format=encoded&counter=-1&format=hex');
  await expect(page.locator('#tool-output')).toContainText('non-negative whole number', { timeout: 15000 });
});
