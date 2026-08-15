import { test, expect } from './fixtures';

const tool = '/tools/stopword-filter/';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('stopword-filter page removes English stop words with exact output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'This is a test of the emergency broadcast system.');
  await page.selectOption('#in-language', 'english');
  await page.selectOption('#in-output', 'text');

  await expect(page.locator('#tool-output')).toHaveText('test emergency broadcast system.', {
    timeout: 15000,
  });
});

test('stopword-filter page supports custom list only and non-default checkbox', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'Buy cheap widgets now!');
  await page.selectOption('#in-language', 'none');
  await page.fill('#in-custom_words', 'cheap, now');
  await page.check('#in-remove_punctuation');

  await expect(page.locator('#tool-output')).toHaveText('Buy widgets', { timeout: 15000 });
});

test('stopword-filter page lists removed words and keeps protected words', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'to be or not to be');
  await page.fill('#in-keep_words', 'not');
  await page.selectOption('#in-output', 'removed');

  await expect(page.locator('#tool-output')).toHaveText('2\tto\n2\tbe\n1\tor', { timeout: 15000 });
});

test('stopword-filter query-param deep-link prefills and computes stats', async ({ page }) => {
  await page.goto(
    tool +
      '?text=' +
      encodeURIComponent('the cat sat on the mat') +
      '&language=english&output=stats&case_sensitive=false&remove_punctuation=false',
  );

  await expect(page.locator('#in-text')).toHaveValue('the cat sat on the mat', { timeout: 15000 });
  await expect(page.locator('#in-language')).toHaveValue('english');
  await expect(page.locator('#in-output')).toHaveValue('stats');
  expect(await outputText(page)).toBe(
    'Total words: 6\nRemoved: 3 (50.00%)\nKept: 3\nDistinct stop words removed: 2',
  );
});
