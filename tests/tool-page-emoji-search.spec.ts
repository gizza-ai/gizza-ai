import { test, expect } from './fixtures';

test('emoji-search page finds emoji by shortcode', async ({ page }) => {
  await page.goto('/tools/emoji-search/');
  await page.fill('#in-query', ':rocket:');
  await page.fill('#in-limit', '3');
  // The labelled output line includes the rocket glyph + its shortcode.
  await expect(page.locator('#tool-output')).toContainText('🚀', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText(':rocket:');
});

test('emoji-search page glyphs-only checkbox returns bare glyphs', async ({ page }) => {
  await page.goto('/tools/emoji-search/');
  await page.fill('#in-query', 'flags');
  await page.fill('#in-limit', '5');
  await page.check('#in-glyphs_only');
  // Glyphs-only output has no shortcode colons.
  await expect(page.locator('#tool-output')).toContainText('🏁', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).not.toContainText(':');
});

test('emoji-search query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/emoji-search/?query=heart&limit=4');
  await expect(page.locator('#in-query')).toHaveValue('heart', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('❤️', {
    timeout: 15000,
  });
});
