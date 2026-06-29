import { test, expect } from './fixtures';

test('tail-lines page keeps the last N lines', async ({ page }) => {
  await page.goto('/tools/tail-lines/');
  await page.fill('#in-text', 'a\nb\nc\nd\ne');
  await page.fill('#in-count', '3');
  await expect(page.locator('#tool-output')).toHaveText('c\nd\ne', { timeout: 15000 });
});

test('tail-lines page supports skip + line numbering', async ({ page }) => {
  await page.goto('/tools/tail-lines/');
  await page.fill('#in-text', 'w\nx\ny\nz');
  await page.fill('#in-count', '2');
  await page.fill('#in-skip', '1');
  await page.check('#in-number');
  await expect(page.locator('#tool-output')).toHaveText('2\tx\n3\ty', { timeout: 15000 });
});
