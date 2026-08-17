import { test, expect } from './fixtures';

test('one-hot-encoder page expands a city column exactly', async ({ page }) => {
  await page.goto('/tools/one-hot-encoder/');
  await page.fill('#in-data', 'city,n\nParis,1\nRome,2\nParis,3');
  await page.fill('#in-column', 'city');
  await page.fill('#in-prefix', '');
  await page.fill('#in-separator', '_');
  await page.selectOption('#in-drop', 'none');
  await page.check('#in-drop_original');
  await page.selectOption('#in-missing', 'zeros');
  await page.fill('#in-max_categories', '0');
  await page.fill('#in-min_count', '0');
  await page.uncheck('#in-other_column');
  await page.fill('#in-positive', '1');
  await page.fill('#in-negative', '0');
  await page.check('#in-case_sensitive');
  await page.selectOption('#in-sort', 'alphabetical');
  await page.check('#in-has_header');
  await page.selectOption('#in-delimiter', 'comma');
  await expect(page.locator('#tool-output')).toHaveText('n,city_Paris,city_Rome\n1,1,0\n2,0,1\n3,1,0', { timeout: 15_000 });
});

test('one-hot-encoder deep link keeps source and drops a reference level', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'city,n\nParis,1\nRome,2\nOslo,3\nParis,4',
    column: 'city',
    prefix: '',
    separator: '_',
    drop: 'first',
    drop_original: 'false',
    missing: 'zeros',
    max_categories: '0',
    min_count: '0',
    other_column: 'false',
    positive: '1',
    negative: '0',
    case_sensitive: 'true',
    sort: 'alphabetical',
    has_header: 'true',
    delimiter: 'comma',
  });
  await page.goto(`/tools/one-hot-encoder/?${params.toString()}`);
  await expect(page.locator('#in-drop')).toHaveValue('first', { timeout: 15_000 });
  await expect(page.locator('#in-drop_original')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('city,n,city_Paris,city_Rome\nParis,1,1,0\nRome,2,0,1\nOslo,3,0,0\nParis,4,1,0', { timeout: 15_000 });
});

test('one-hot-encoder page groups rare categories into other', async ({ page }) => {
  await page.goto('/tools/one-hot-encoder/');
  await page.fill('#in-data', 'browser,hits\nchrome,10\nchrome,20\nchrome,30\nsafari,40\nsafari,50\nlynx,60');
  await page.fill('#in-column', 'browser');
  await page.selectOption('#in-sort', 'frequency');
  await page.fill('#in-max_categories', '2');
  await page.check('#in-other_column');
  await page.uncheck('#in-drop_original');
  await expect(page.locator('#tool-output')).toHaveText('browser,hits,browser_chrome,browser_safari,browser_other\nchrome,10,1,0,0\nchrome,20,1,0,0\nchrome,30,1,0,0\nsafari,40,0,1,0\nsafari,50,0,1,0\nlynx,60,0,0,1', { timeout: 15_000 });
});
