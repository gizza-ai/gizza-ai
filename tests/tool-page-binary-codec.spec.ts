import { test, expect } from './fixtures';

test('binary-codec page encodes text to binary with defaults', async ({ page }) => {
  await page.goto('/tools/binary-codec/');
  await page.fill('#in-input', 'Hi');
  // Default delimiter is a space.
  await expect(page.locator('#tool-output')).toHaveText('01001000 01101001', { timeout: 15000 });
});

test('binary-codec page decodes a binary string back to text', async ({ page }) => {
  await page.goto('/tools/binary-codec/');
  await page.fill('#in-input', '01001000 01101001');
  await page.selectOption('#in-mode', 'decode');
  await page.selectOption('#in-format', 'text');
  await expect(page.locator('#tool-output')).toHaveText('Hi', { timeout: 15000 });
});

test('binary-codec page honours delimiter and 0b prefix on encode', async ({ page }) => {
  await page.goto('/tools/binary-codec/');
  await page.fill('#in-input', 'Hi');
  await page.selectOption('#in-delimiter', 'colon');
  await expect(page.locator('#tool-output')).toHaveText('01001000:01101001', { timeout: 15000 });

  await page.selectOption('#in-delimiter', 'space');
  await page.selectOption('#in-prefix', '0b');
  await expect(page.locator('#tool-output')).toHaveText('0b01001000 0b01101001', { timeout: 15000 });
});

test('binary-codec page decodes non-UTF-8 bytes as hex', async ({ page }) => {
  await page.goto('/tools/binary-codec/');
  await page.fill('#in-input', '11011110 10101101 10111110 11101111');
  await page.selectOption('#in-mode', 'decode');
  await page.selectOption('#in-format', 'bytes');
  await expect(page.locator('#tool-output')).toHaveText('deadbeef', { timeout: 15000 });
});

test('binary-codec page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/binary-codec/?input=01001000%2001101001&mode=decode&format=text');
  await expect(page.locator('#in-input')).toHaveValue('01001000 01101001');
  await expect(page.locator('#in-mode')).toHaveValue('decode');
  await expect(page.locator('#tool-output')).toHaveText('Hi', { timeout: 15000 });
});
