import { test, expect } from './fixtures';

// /tools/extract-hashes/ extracts + groups hex hashes by algorithm (pure wasm).
test('extract-hashes groups hashes by algorithm', async ({ page }) => {
  await page.goto('/tools/extract-hashes/');
  await page.fill(
    '#in-text',
    'md5 d41d8cd98f00b204e9800998ecf8427e and sha256 ' +
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 ' +
      'dup d41d8cd98f00b204e9800998ecf8427e',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 unique hash(es)', { timeout: 15000 });
  await expect(out).toContainText('MD5 (32 chars)');
  await expect(out).toContainText('SHA256 (64 chars)');
  await expect(out).toContainText('d41d8cd98f00b204e9800998ecf8427e');
});

test('extract-hashes preserves casing when lowercase unchecked', async ({ page }) => {
  await page.goto('/tools/extract-hashes/');
  await page.fill('#in-text', 'D41D8CD98F00B204E9800998ECF8427E');
  await page.uncheck('#in-lowercase');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('D41D8CD98F00B204E9800998ECF8427E', { timeout: 15000 });
});
