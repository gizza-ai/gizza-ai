import { test, expect } from './fixtures';

test('morse-code page encodes text by default', async ({ page }) => {
  await page.goto('/tools/morse-code/');
  await page.fill('#in-text', 'SOS');
  await expect(page.locator('#tool-output')).toHaveText('... --- ...', { timeout: 15000 });
});

test('morse-code page decodes morse back to text', async ({ page }) => {
  await page.goto('/tools/morse-code/');
  await page.fill('#in-text', '.... . .-.. .-.. --- / .-- --- .-. .-.. -..');
  await page.selectOption('#in-direction', 'decode');
  await expect(page.locator('#tool-output')).toHaveText('HELLO WORLD', { timeout: 15000 });
});

test('morse-code page supports custom separators', async ({ page }) => {
  await page.goto('/tools/morse-code/');
  await page.fill('#in-text', 'HI YO');
  await page.fill('#in-letter_sep', '|');
  await page.fill('#in-word_sep', '//');
  await expect(page.locator('#tool-output')).toHaveText('....|..//-.--|---', { timeout: 15000 });
});

test('morse-code query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/morse-code/?text=' + encodeURIComponent('SOS'));
  await expect(page.locator('#in-text')).toHaveValue('SOS', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('... --- ...', { timeout: 15000 });
});
