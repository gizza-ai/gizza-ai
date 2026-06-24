import { test, expect } from './fixtures';

// /tools/evp-derive/ reproduces OpenSSL's EVP_BytesToKey in-browser (pure wasm).
// password/salt/key_length/iv_length/count are fields; salt_encoding/hash/encoding
// are <select>s (rendered from Param::enumv). Output is "key: <hex>\niv:  <hex>".
test('evp-derive page matches an OpenSSL SHA-256 vector', async ({ page }) => {
  await page.goto('/tools/evp-derive/');
  await page.fill('#in-password', 'password');
  await page.fill('#in-salt', '');
  await page.selectOption('#in-salt_encoding', 'utf8');
  await page.selectOption('#in-hash', 'sha256');
  await page.fill('#in-key_length', '32');
  await page.fill('#in-iv_length', '16');
  await page.fill('#in-count', '1');
  await page.selectOption('#in-encoding', 'hex');
  const out = page.locator('#tool-output');
  // openssl enc -aes-256-cbc -md sha256 -nosalt -pass pass:password -P
  await expect(out).toContainText(
    '5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8',
    { timeout: 15000 },
  );
  await expect(out).toContainText('3b02902846ffd32e92ff168b3f5d16b0');
});

test('evp-derive page matches an OpenSSL MD5+salt vector', async ({ page }) => {
  await page.goto('/tools/evp-derive/');
  await page.fill('#in-password', 'hello');
  await page.fill('#in-salt', '0102030405060708');
  await page.selectOption('#in-salt_encoding', 'hex');
  await page.selectOption('#in-hash', 'md5');
  await page.fill('#in-key_length', '32');
  await page.fill('#in-iv_length', '16');
  await page.fill('#in-count', '1');
  await page.selectOption('#in-encoding', 'hex');
  const out = page.locator('#tool-output');
  // openssl enc -aes-256-cbc -md md5 -S 0102030405060708 -pass pass:hello -P
  await expect(out).toContainText(
    '577943ad91305815e2bd7ed2d805eefab293e5d367f40c7dcb692b5f41bb5e08',
    { timeout: 15000 },
  );
});

test('evp-derive page deep-links via query params', async ({ page }) => {
  await page.goto('/tools/evp-derive/?password=password&hash=md5&key_length=16&iv_length=0&encoding=hex');
  const out = page.locator('#tool-output');
  // MD5("password") = single-block key, no IV.
  await expect(out).toContainText('5f4dcc3b5aa765d61d8327deb882cf99', { timeout: 15000 });
});
