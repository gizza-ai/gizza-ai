import { test, expect } from './fixtures';

// /tools/aes-key-wrap/ wraps/unwraps key material with AES Key Wrap (pure wasm).
// data/key are multiline <textarea>; operation/algorithm/format are <select>.
const KEK = '000102030405060708090a0b0c0d0e0f'; // AES-128 KEK, hex
const KEY = '00112233445566778899aabbccddeeff'; // 128-bit key material, hex
const WRAPPED = '1fa68b0a8112b447aef34bd8fb5a7b829d3e862371d2cfe5'; // RFC 3394 vector

test('aes-key-wrap page wraps the RFC 3394 vector, then unwraps it', async ({ page }) => {
  await page.goto('/tools/aes-key-wrap/');
  await page.fill('#in-data', KEY);
  await page.selectOption('#in-operation', 'wrap');
  await page.fill('#in-key', KEK);
  await page.selectOption('#in-algorithm', 'kw');
  await page.selectOption('#in-format', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText(WRAPPED, { timeout: 15000 });

  await page.fill('#in-data', WRAPPED);
  await page.selectOption('#in-operation', 'unwrap');
  await expect(out).toHaveText(KEY, { timeout: 15000 });
});

test('aes-key-wrap page rejects a bad KEK length', async ({ page }) => {
  await page.goto('/tools/aes-key-wrap/');
  await page.fill('#in-data', KEY);
  await page.selectOption('#in-operation', 'wrap');
  await page.fill('#in-key', 'abcd'); // 2 bytes — too short
  await page.selectOption('#in-algorithm', 'kw');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('16, 24, or 32 bytes', { timeout: 15000 });
});

test('aes-key-wrap page prefills from query params and computes', async ({ page }) => {
  await page.goto(
    `/tools/aes-key-wrap/?data=${KEY}&operation=wrap&key=${KEK}&algorithm=kw&format=hex`,
  );
  await expect(page.locator('#in-data')).toHaveValue(KEY);
  await expect(page.locator('#in-key')).toHaveValue(KEK);
  await expect(page.locator('#tool-output')).toHaveText(WRAPPED, { timeout: 15000 });
});
