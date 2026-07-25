import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const DEFAULT_EXPECTED = [
  'Merged (4):',
  'apple',
  'banana',
  'cherry',
  'date',
  '',
  'Totals: A=3 · B=3 · merged=4 · duplicates removed=2 · shared by both=2',
].join('\n');

test('list-dedupe-merge page — default append union with exact counts', async ({ page }) => {
  await page.goto('/tools/list-dedupe-merge/');
  await page.fill('#in-list_a', 'apple\nbanana\ncherry');
  await page.fill('#in-list_b', 'banana\ncherry\ndate');
  await expect(page.locator('#tool-output')).toContainText('shared by both=2', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});

test('list-dedupe-merge page — interleave order and ignore case checkbox', async ({ page }) => {
  await page.goto('/tools/list-dedupe-merge/');
  await page.fill('#in-list_a', 'A\nBob@x.com\nC');
  await page.fill('#in-list_b', 'bob@x.com\nD\nE');
  await page.selectOption('#in-merge_order', 'interleave');
  await page.check('#in-ignore_case');
  await expect(page.locator('#tool-output')).toContainText('duplicates removed=1', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'Merged (5):',
      'A',
      'bob@x.com',
      'D',
      'C',
      'E',
      '',
      'Totals: A=3 · B=3 · merged=5 · duplicates removed=1 · shared by both=1',
    ].join('\n'),
  );
});

test('list-dedupe-merge page — comma separator and A to Z sort', async ({ page }) => {
  await page.goto('/tools/list-dedupe-merge/');
  await page.fill('#in-list_a', 'delta,alpha,charlie');
  await page.fill('#in-list_b', 'alpha,bravo');
  await page.selectOption('#in-separator', 'comma');
  await page.selectOption('#in-sort', 'asc');
  await expect(page.locator('#tool-output')).toContainText('merged=4', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'Merged (4):',
      'alpha',
      'bravo',
      'charlie',
      'delta',
      '',
      'Totals: A=3 · B=2 · merged=4 · duplicates removed=1 · shared by both=1',
    ].join('\n'),
  );
});

test('list-dedupe-merge page — ignore leading zeros checkbox', async ({ page }) => {
  await page.goto('/tools/list-dedupe-merge/');
  await page.fill('#in-list_a', '007\n042\n100');
  await page.fill('#in-list_b', '7\n100\n250');
  await page.check('#in-ignore_leading_zeros');
  await expect(page.locator('#tool-output')).toContainText('shared by both=2', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'Merged (4):',
      '007',
      '042',
      '100',
      '250',
      '',
      'Totals: A=3 · B=3 · merged=4 · duplicates removed=2 · shared by both=2',
    ].join('\n'),
  );
});

test('list-dedupe-merge page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/list-dedupe-merge/?list_a=' +
      encodeURIComponent('apple\nbanana\ncherry') +
      '&list_b=' +
      encodeURIComponent('banana\ncherry\ndate') +
      '&separator=newline&merge_order=append&trim=true&ignore_blank=true&ignore_case=false&sort=input&ignore_leading_zeros=false',
  );
  await expect(page.locator('#in-list_a')).toHaveValue('apple\nbanana\ncherry', { timeout: 15000 });
  await expect(page.locator('#in-list_b')).toHaveValue('banana\ncherry\ndate');
  await expect(page.locator('#tool-output')).toContainText('shared by both=2', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});
