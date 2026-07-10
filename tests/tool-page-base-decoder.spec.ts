import { test, expect } from './fixtures';

test('base-decoder peels nested Base64 layers and reports the chain', async ({ page }) => {
  await page.goto('/tools/base-decoder/');
  // Defaults: max_depth=8, output=report. base64(base64("Hello, World!")).
  await page.fill('#in-input', 'U0dWc2JHOHNJRmR2Y214a0lRPT0=');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Detected 2 layer(s): base64 → base64', {
    timeout: 15000,
  });
  await expect(out).toContainText('Hello, World!');
});

test('base-decoder plain output returns only the decoded text', async ({ page }) => {
  await page.goto('/tools/base-decoder/');
  await page.fill('#in-input', 'SGVsbG8sIFdvcmxkIQ==');
  await page.selectOption('#in-output', 'plain');
  await expect(page.locator('#tool-output')).toHaveText('Hello, World!', {
    timeout: 15000,
  });
});

test('base-decoder detects a single Base32 layer', async ({ page }) => {
  await page.goto('/tools/base-decoder/');
  // base32("hello world")
  await page.fill('#in-input', 'NBSWY3DPEB3W64TMMQ======');
  await page.selectOption('#in-output', 'plain');
  await expect(page.locator('#tool-output')).toHaveText('hello world', {
    timeout: 15000,
  });
});

test('base-decoder max_depth cap stops after one layer', async ({ page }) => {
  await page.goto('/tools/base-decoder/');
  await page.fill('#in-input', 'U0dWc2JHOHNJRmR2Y214a0lRPT0=');
  await page.fill('#in-max_depth', '1');
  await page.selectOption('#in-output', 'plain');
  // With one peel, the inner Base64 string is left un-decoded.
  await expect(page.locator('#tool-output')).toHaveText('SGVsbG8sIFdvcmxkIQ==', {
    timeout: 15000,
  });
});

test('base-decoder recognizes a binary file signature (PNG)', async ({ page }) => {
  await page.goto('/tools/base-decoder/');
  // hex of a PNG header — a Base16 layer that decodes to a known binary target.
  await page.fill('#in-input', '89504e470d0a1a0a0000000d49484452');
  await expect(page.locator('#tool-output')).toContainText('PNG image', {
    timeout: 15000,
  });
});

test('base-decoder query-param deep-link prefills controls and decodes', async ({ page }) => {
  await page.goto('/tools/base-decoder/?input=SGVsbG8sIFdvcmxkIQ%3D%3D&output=plain');
  await expect(page.locator('#in-input')).toHaveValue('SGVsbG8sIFdvcmxkIQ==', {
    timeout: 15000,
  });
  await expect(page.locator('#in-output')).toHaveValue('plain');
  await expect(page.locator('#tool-output')).toHaveText('Hello, World!', {
    timeout: 15000,
  });
});
