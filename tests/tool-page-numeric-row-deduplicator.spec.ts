import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const DEFAULT_INPUT = '1,2\n1.0,2.0\n1.00,2\n3,4';
const DEFAULT_EXPECTED = '1,2\n3,4';

test('numeric-row-deduplicator page — collapses numeric representations', async ({ page }) => {
  await page.goto('/tools/numeric-row-deduplicator/');
  await page.fill('#in-data', DEFAULT_INPUT);
  await expect(page.locator('#tool-output')).toContainText('3,4', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});

test('numeric-row-deduplicator page — keys on a named column with header', async ({ page }) => {
  await page.goto('/tools/numeric-row-deduplicator/');
  await page.fill('#in-data', 'id,score\n1,90.0\n1,91\n2,80');
  await page.fill('#in-columns', 'id');
  await page.check('#in-header');
  await expect(page.locator('#tool-output')).toContainText('id,score', { timeout: 15000 });
  expect(await outputText(page)).toBe('id,score\n1,90.0\n2,80');
});

test('numeric-row-deduplicator page — keep-last enum preserves survivor order', async ({ page }) => {
  await page.goto('/tools/numeric-row-deduplicator/');
  await page.fill('#in-data', '1,a\n2,b\n1.0,c\n3,d');
  await page.fill('#in-columns', '1');
  await page.selectOption('#in-keep', 'last');
  await expect(page.locator('#tool-output')).toContainText('1.0,c', { timeout: 15000 });
  expect(await outputText(page)).toBe('2,b\n1.0,c\n3,d');
});

test('numeric-row-deduplicator page — precision tolerance and tab delimiter', async ({ page }) => {
  await page.goto('/tools/numeric-row-deduplicator/');
  await page.fill('#in-data', '0.30000000000000004\tA\n0.3\tA\n0.31\tA');
  await page.fill('#in-delimiter', 'tab');
  await page.fill('#in-precision', '2');
  await expect(page.locator('#tool-output')).toContainText('0.31\tA', { timeout: 15000 });
  expect(await outputText(page)).toBe('0.30000000000000004\tA\n0.31\tA');
});

test('numeric-row-deduplicator page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/numeric-row-deduplicator/?data=' +
      encodeURIComponent(DEFAULT_INPUT) +
      '&columns=&header=false&delimiter=' +
      encodeURIComponent(',') +
      '&precision=-1&keep=first',
  );
  await expect(page.locator('#in-data')).toHaveValue(DEFAULT_INPUT, { timeout: 15000 });
  await expect(page.locator('#in-keep')).toHaveValue('first');
  await expect(page.locator('#tool-output')).toContainText('3,4', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});
