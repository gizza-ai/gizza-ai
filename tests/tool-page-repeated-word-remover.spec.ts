import { test, expect } from './fixtures';

// /tools/repeated-word-remover/ cleans adjacent doubled words in-browser.
test('repeated-word-remover page cleans doubled words exactly', async ({ page }) => {
  await page.goto('/tools/repeated-word-remover/');
  await page.fill('#in-input', 'I think the the cat sat on on the mat, and he had had enough.');
  await page.selectOption('#in-output', 'clean');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('I think the cat sat on the mat, and he had had enough.', { timeout: 15000 });
});

test('repeated-word-remover page deep-link renders the audit report', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('the the cat\nsat on on the mat') +
    '&output=report' +
    '&keep_words=' + encodeURIComponent('had, that, is') +
    '&case_sensitive=false&across_line_breaks=true&ignore_punctuation=false&include_numbers=false&min_length=1';
  await page.goto('/tools/repeated-word-remover/' + qs);

  await expect(page.locator('#in-output')).toHaveValue('report', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Found 2 doubled-word spots; removed 2 word copies.', { timeout: 15000 });
  await expect(out).toContainText('line 1, col 1: "the the" → "the"');
  await expect(out).toContainText('line 2, col 5: "on on" → "on"');
});

test('repeated-word-remover page exercises non-default controls', async ({ page }) => {
  await page.goto('/tools/repeated-word-remover/');
  await page.fill('#in-input', 'Well, well now. 2024 2024 rows. I I saw the the sign.');
  await page.selectOption('#in-output', 'marked');
  await page.check('#in-ignore_punctuation');
  await page.check('#in-include_numbers');
  await page.fill('#in-min_length', '3');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Well, ~~well~~ now. 2024 ~~2024~~ rows. I I saw the ~~the~~ sign.', { timeout: 15000 });
});
