import { test, expect } from './fixtures';

test('head-lines page keeps the first N lines', async ({ page }) => {
  await page.goto('/tools/head-lines/');
  await page.fill('#in-text', 'a\nb\nc\nd\ne');
  await page.fill('#in-count', '3');
  await expect(page.locator('#tool-output')).toHaveText('a\nb\nc', { timeout: 15000 });
});

test('head-lines page supports skip + line numbering', async ({ page }) => {
  await page.goto('/tools/head-lines/');
  await page.fill('#in-text', 'header\nx\ny\nz');
  await page.fill('#in-count', '2');
  await page.fill('#in-skip', '1');
  await page.check('#in-number');
  await expect(page.locator('#tool-output')).toHaveText('2\tx\n3\ty', { timeout: 15000 });
});
