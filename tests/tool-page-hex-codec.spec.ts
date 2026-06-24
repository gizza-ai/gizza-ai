import { test, expect } from './fixtures';

test('hex-codec encodes text with default settings', async ({ page }) => {
  await page.goto('/tools/hex-codec/');
  // Defaults: mode=encode, delimiter=none, lowercase, no prefix.
  await page.fill('#in-input', 'Hi');
  await expect(page.locator('#tool-output')).toHaveText('4869', {
    timeout: 15000,
  });
});

test('hex-codec decodes hex back to text', async ({ page }) => {
  await page.goto('/tools/hex-codec/');
  await page.fill('#in-input', '4869');
  await page.selectOption('#in-mode', 'decode');
  await expect(page.locator('#tool-output')).toHaveText('Hi', {
    timeout: 15000,
  });
});

test('hex-codec colon delimiter + uppercase', async ({ page }) => {
  await page.goto('/tools/hex-codec/');
  await page.fill('#in-input', 'Hello');
  await page.selectOption('#in-delimiter', 'colon');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText('48:65:6C:6C:6F', {
    timeout: 15000,
  });
});

test('hex-codec 0x prefix with space delimiter', async ({ page }) => {
  await page.goto('/tools/hex-codec/');
  await page.fill('#in-input', 'Hi');
  await page.selectOption('#in-prefix', '0x');
  await page.selectOption('#in-delimiter', 'space');
  await expect(page.locator('#tool-output')).toHaveText('0x48 0x69', {
    timeout: 15000,
  });
});

test('hex-codec decode tolerates delimiters and prefixes', async ({ page }) => {
  await page.goto('/tools/hex-codec/');
  await page.fill('#in-input', '48:65:6c 6c 6f');
  await page.selectOption('#in-mode', 'decode');
  await expect(page.locator('#tool-output')).toHaveText('Hello', {
    timeout: 15000,
  });
});

test('hex-codec decode to raw bytes format', async ({ page }) => {
  await page.goto('/tools/hex-codec/');
  // 0xde 0xad 0xbe 0xef is not valid UTF-8 — bytes format shows the hex.
  await page.fill('#in-input', 'deadbeef');
  await page.selectOption('#in-mode', 'decode');
  await page.selectOption('#in-format', 'bytes');
  await expect(page.locator('#tool-output')).toHaveText('deadbeef', {
    timeout: 15000,
  });
});

test('hex-codec query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/hex-codec/?input=Hello&delimiter=space');
  await expect(page.locator('#in-input')).toHaveValue('Hello', {
    timeout: 15000,
  });
  await expect(page.locator('#in-delimiter')).toHaveValue('space');
  await expect(page.locator('#tool-output')).toHaveText('48 65 6c 6c 6f', {
    timeout: 15000,
  });
});
