import { test, expect } from './fixtures';

test('text-corpus-cleaner page trims and exact-dedupes lines by default', async ({ page }) => {
  await page.goto('/tools/text-corpus-cleaner/');
  await page.fill('#in-input', '  Hello World  \nHello World\nfoo bar\nHello World');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('foo bar', { timeout: 15_000 });
  // Trim strips the edges, exact dedupe keeps the first "Hello World".
  expect(await out.textContent()).toBe('Hello World\nfoo bar');
});

test('text-corpus-cleaner deep link applies collapse + normalized dedupe', async ({ page }) => {
  const corpus = 'The Quick  Brown Fox\nthe quick brown fox\nThe Quick  Brown Fox';
  const qs =
    '?input=' + encodeURIComponent(corpus) +
    '&whitespace=collapse' +
    '&dedupe=normalized';
  await page.goto('/tools/text-corpus-cleaner/' + qs);

  await expect(page.locator('#in-whitespace')).toHaveValue('collapse', { timeout: 15_000 });
  await expect(page.locator('#in-dedupe')).toHaveValue('normalized');

  // All three fold to the same key; only the first survives, verbatim after collapse.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Brown', { timeout: 15_000 });
  expect(await out.textContent()).toBe('The Quick Brown Fox');
});

test('text-corpus-cleaner deep link filters short and mostly-symbol lines', async ({ page }) => {
  const corpus = 'a real sentence here\nok\n===== >>>>> #####\nanother good one';
  const qs =
    '?input=' + encodeURIComponent(corpus) +
    '&min_words=3' +
    '&max_symbol_ratio=0.5';
  await page.goto('/tools/text-corpus-cleaner/' + qs);

  await expect(page.locator('#in-min_words')).toHaveValue('3', { timeout: 15_000 });
  await expect(page.locator('#in-max_symbol_ratio')).toHaveValue('0.5');
  // collapse_blank defaults on.
  await expect(page.locator('#in-collapse_blank')).toBeChecked();

  // "ok" has too few words; the separator line is all symbols (ratio 1.0 > 0.5).
  const out = page.locator('#tool-output');
  await expect(out).toContainText('another good one', { timeout: 15_000 });
  expect(await out.textContent()).toBe('a real sentence here\nanother good one');
});
