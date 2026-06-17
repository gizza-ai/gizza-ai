import { test, expect } from './fixtures';

test('word-count page', async ({ page }) => {
  await page.goto('/tools/word-count/');
  await page.fill('#in-text', 'one two three');
  await expect(page.locator('#tool-output')).toContainText('3 words', { timeout: 15000 });
});
