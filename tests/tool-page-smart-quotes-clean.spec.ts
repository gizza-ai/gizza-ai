import { test, expect } from './fixtures';

test('smart-quotes-clean straightens punctuation', async ({ page }) => {
  await page.goto('/tools/smart-quotes-clean/');
  await page.fill('#in-text', '“Hello”—it’s fine…');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('"Hello"--it\'s fine...', { timeout: 15000 });
});

test('smart-quotes-clean em dash option and space normalization toggle', async ({ page }) => {
  await page.goto('/tools/smart-quotes-clean/');
  await page.fill('#in-text', 'a\u00A0b—c');
  await page.selectOption('#in-em_dash', '-');
  await page.uncheck('#in-normalize_spaces');
  await expect(page.locator('#tool-output')).toHaveText('a\u00A0b-c', { timeout: 15000 });
});

test('smart-quotes-clean query-param deep-link', async ({ page }) => {
  await page.goto(
    '/tools/smart-quotes-clean/?text=' +
      encodeURIComponent('«yes»\u2026') +
      '&em_dash=-&normalize_spaces=true',
  );
  await expect(page.locator('#in-text')).toHaveValue('«yes»…', { timeout: 15000 });
  await expect(page.locator('#in-em_dash')).toHaveValue('-');
  await expect(page.locator('#in-normalize_spaces')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('"yes"...', { timeout: 15000 });
});
