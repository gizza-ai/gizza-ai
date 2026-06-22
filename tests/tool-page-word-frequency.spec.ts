import { test, expect } from './fixtures';

// /tools/word-frequency/ tallies word frequencies in-browser (pure wasm).
test('word-frequency ranks words by occurrence', async ({ page }) => {
  await page.goto('/tools/word-frequency/');
  await page.fill('#in-text', 'red blue red green red blue');
  const out = page.locator('#tool-output');
  // rows are "count<TAB>percent<TAB>word", most-frequent first.
  await expect(out).toContainText('3\t50.00%\tred', { timeout: 15000 });
  await expect(out).toContainText('2\t33.33%\tblue');
  await expect(out).toContainText('1\t16.67%\tgreen');
});

test('word-frequency honors stop words and top-N options', async ({ page }) => {
  await page.goto('/tools/word-frequency/');
  await page.fill('#in-text', 'the cat sat on the mat the cat ran fast cat');
  // ignore_stopwords + top renders as checkbox + number fields.
  await page.check('#in-ignore_stopwords');
  await page.fill('#in-top', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3\t', { timeout: 15000 });
  await expect(out).toContainText('\tcat');
  // stop words like "the"/"on" are dropped, so they must not appear.
  await expect(out).not.toContainText('\tthe');
  await expect(out).not.toContainText('\ton');
});
