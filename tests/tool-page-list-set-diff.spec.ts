import { test, expect } from './fixtures';

// Output is multi-line, so assert exact textContent (toHaveText collapses
// whitespace and can't verify the newline structure).
async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const DEFAULT_EXPECTED = [
  'Only in A (1):',
  'apple',
  '',
  'Only in B (1):',
  'date',
  '',
  'In both (2):',
  'banana',
  'cherry',
  '',
  'Totals: A=3 · B=3 · only in A=1 · only in B=1 · in both=2 · union=4',
].join('\n');

test('list-set-diff page — default comparison', async ({ page }) => {
  await page.goto('/tools/list-set-diff/');
  await page.fill('#in-list_a', 'apple\nbanana\ncherry');
  await page.fill('#in-list_b', 'banana\ncherry\ndate');
  await expect(page.locator('#tool-output')).toContainText('union=4', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});

test('list-set-diff page — ignore case checkbox (non-default state)', async ({ page }) => {
  await page.goto('/tools/list-set-diff/');
  await page.fill('#in-list_a', 'Alice@x.com\nBob@x.com\ncarol@x.com');
  await page.fill('#in-list_b', 'bob@x.com\ndave@x.com');
  await page.check('#in-ignore_case');
  await expect(page.locator('#tool-output')).toContainText('In both (1):', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'Only in A (2):',
      'Alice@x.com',
      'carol@x.com',
      '',
      'Only in B (1):',
      'dave@x.com',
      '',
      'In both (1):',
      'Bob@x.com',
      '',
      'Totals: A=3 · B=2 · only in A=2 · only in B=1 · in both=1 · union=4',
    ].join('\n'),
  );
});

test('list-set-diff page — comma separator + A→Z sort', async ({ page }) => {
  await page.goto('/tools/list-set-diff/');
  await page.fill('#in-list_a', 'delta,alpha,charlie');
  await page.fill('#in-list_b', 'alpha,bravo');
  await page.selectOption('#in-separator', 'comma');
  await page.selectOption('#in-sort', 'asc');
  await expect(page.locator('#tool-output')).toContainText('In both (1):', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'Only in A (2):',
      'charlie',
      'delta',
      '',
      'Only in B (1):',
      'bravo',
      '',
      'In both (1):',
      'alpha',
      '',
      'Totals: A=3 · B=2 · only in A=2 · only in B=1 · in both=1 · union=4',
    ].join('\n'),
  );
});

test('list-set-diff page — ignore leading zeros checkbox', async ({ page }) => {
  await page.goto('/tools/list-set-diff/');
  await page.fill('#in-list_a', '007\n042\n100');
  await page.fill('#in-list_b', '7\n100\n250');
  await page.check('#in-ignore_leading_zeros');
  await expect(page.locator('#tool-output')).toContainText('In both (2):', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'Only in A (1):',
      '042',
      '',
      'Only in B (1):',
      '250',
      '',
      'In both (2):',
      '007',
      '100',
      '',
      'Totals: A=3 · B=3 · only in A=1 · only in B=1 · in both=2 · union=4',
    ].join('\n'),
  );
});

test('list-set-diff page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/list-set-diff/?list_a=' +
      encodeURIComponent('apple\nbanana\ncherry') +
      '&list_b=' +
      encodeURIComponent('banana\ncherry\ndate'),
  );
  await expect(page.locator('#in-list_a')).toHaveValue('apple\nbanana\ncherry', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('union=4', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});
