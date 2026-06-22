import { test, expect } from './fixtures';

test('byte-take page extracts a byte range', async ({ page }) => {
  await page.goto('/tools/byte-take/');
  await page.fill('#in-input', 'Hello, World');
  await page.fill('#in-start', '5');
  await page.fill('#in-length', '4');
  await page.selectOption('#in-format', 'text');
  await page.selectOption('#in-output', 'text');
  await expect(page.locator('#tool-output')).toHaveText(', Wo', { timeout: 15000 });
});

test('byte-take page handles hex in/out', async ({ page }) => {
  await page.goto('/tools/byte-take/');
  await page.fill('#in-input', '00112233');
  await page.fill('#in-start', '1');
  await page.fill('#in-length', '2');
  await page.selectOption('#in-format', 'hex');
  await page.selectOption('#in-output', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('1122', { timeout: 15000 });
});
