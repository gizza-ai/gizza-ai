import { test, expect } from './fixtures';

test('tweet-thread-splitter page splits into numbered tweets', async ({ page }) => {
  await page.goto('/tools/tweet-thread-splitter/');

  await page.fill('#in-text', 'aaaaa bbbbb ccccc ddddd eeeee fffff');
  await page.fill('#in-limit', '20');
  // numbering defaults to "parens"; no sentence punctuation so packing is by word.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('aaaaa bbbbb (1/3)', { timeout: 15000 });
  await expect(out).toContainText('ccccc ddddd (2/3)');
  await expect(out).toContainText('eeeee fffff (3/3)');
});

test('tweet-thread-splitter dotted numbering prepends a number', async ({ page }) => {
  await page.goto('/tools/tweet-thread-splitter/');
  await page.fill('#in-text', 'aaaaa bbbbb ccccc ddddd eeeee fffff');
  await page.fill('#in-limit', '20');
  await page.selectOption('#in-numbering', 'dotted');
  // prefer_sentences checkbox is checked by default; the text has no sentence
  // boundaries, so turn it off explicitly to keep word-packing deterministic.
  await page.uncheck('#in-prefer_sentences');
  await expect(page.locator('#tool-output')).toContainText('1. aaaaa bbbbb ccccc', {
    timeout: 15000,
  });
});

test('tweet-thread-splitter numbering can be turned off', async ({ page }) => {
  await page.goto('/tools/tweet-thread-splitter/');
  await page.fill('#in-text', 'just a short note');
  await page.selectOption('#in-numbering', 'none');
  await expect(page.locator('#tool-output')).toHaveText('just a short note', {
    timeout: 15000,
  });
});

test('tweet-thread-splitter query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/tweet-thread-splitter/?text=' +
      encodeURIComponent('hello world') +
      '&limit=280',
  );
  await expect(page.locator('#in-text')).toHaveValue('hello world', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toHaveText('hello world (1/1)', {
    timeout: 15000,
  });
});
