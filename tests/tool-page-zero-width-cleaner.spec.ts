import { test, expect } from './fixtures';

// /tools/zero-width-cleaner/ strips invisible Unicode formatting characters in-browser.
test('zero-width-cleaner removes zero-width characters by default', async ({ page }) => {
  await page.goto('/tools/zero-width-cleaner/');
  await page.fill('#in-text', 'a\u200Bb\u200Cc\u200Dd\uFEFFe');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('abcde', { timeout: 15000 });
});

test('zero-width-cleaner replacement and NBSP options work', async ({ page }) => {
  await page.goto('/tools/zero-width-cleaner/');
  await page.fill('#in-text', 'a\u200Bb\u00A0c');
  await page.fill('#in-replacement', '_');
  await page.check('#in-replace_nbsp');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('a_b c', { timeout: 15000 });
});

test('zero-width-cleaner can preserve zero-width joiners when disabled', async ({ page }) => {
  await page.goto('/tools/zero-width-cleaner/');
  await page.fill('#in-text', '👨\u200D👩\u200D👧');
  await page.uncheck('#in-remove_zero_width');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('👨‍👩‍👧', { timeout: 15000 });
});
