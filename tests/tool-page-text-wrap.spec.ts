import { test, expect } from './fixtures';

test('text-wrap page wraps to width', async ({ page }) => {
  await page.goto('/tools/text-wrap/');
  await page.fill('#in-text', 'the quick brown fox jumps over the lazy dog');
  await page.fill('#in-width', '15');
  // Output preserves newlines; assert via textContent to keep them.
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), {
      timeout: 15000,
    })
    .toBe('the quick brown\nfox jumps over\nthe lazy dog');
});

test('text-wrap break-long-words checkbox keeps long word intact when off', async ({ page }) => {
  await page.goto('/tools/text-wrap/');
  await page.fill('#in-text', 'supercalifragilisticexpialidocious');
  await page.fill('#in-width', '10');
  // Default on -> hard-split into <=10-char chunks (every line within width).
  await expect
    .poll(
      async () => {
        const t = (await page.locator('#tool-output').textContent())?.trim() ?? '';
        const lines = t.split('\n');
        const allWithin = lines.every((l) => l.length <= 10);
        const rejoins = lines.join('') === 'supercalifragilisticexpialidocious';
        return allWithin && rejoins && lines.length > 1;
      },
      { timeout: 15000 },
    )
    .toBe(true);
  // Turn off -> single over-length line, unchanged.
  await page.uncheck('#in-break_long_words');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), {
      timeout: 15000,
    })
    .toBe('supercalifragilisticexpialidocious');
});

test('text-wrap query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/text-wrap/?text=' +
      encodeURIComponent('the quick brown fox jumps over the lazy dog') +
      '&width=15',
  );
  await expect(page.locator('#in-text')).toHaveValue(
    'the quick brown fox jumps over the lazy dog',
    { timeout: 15000 },
  );
  await expect(page.locator('#in-width')).toHaveValue('15');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), {
      timeout: 15000,
    })
    .toBe('the quick brown\nfox jumps over\nthe lazy dog');
});
