import { test, expect } from './fixtures';

const mapping = 'USA|U.S.A.|us|united states => United States\nCanada|CAN => Canada';

test('category-canonicalize page rewrites a header-selected CSV column exactly', async ({ page }) => {
  await page.goto('/tools/category-canonicalize/');
  await page.fill('#in-data', 'country,n\nUSA,1\nu.s.a.,2\nCanadaa,3\nBrazil,4');
  await page.fill('#in-mapping', mapping);
  await page.fill('#in-column', 'country');
  await page.selectOption('#in-delimiter', 'auto');
  await page.check('#in-header');
  await page.check('#in-ignore_case');
  await page.check('#in-ignore_spacing');
  await page.selectOption('#in-unmatched', 'keep');
  await page.fill('#in-fuzzy_threshold', '85');
  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toHaveText('country,n\nUnited States,1\nUnited States,2\nCanadaa,3\nBrazil,4', { timeout: 15_000 });
});

test('category-canonicalize deep link applies fuzzy suggestions and reflects non-default controls', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'country\nCanadaa\nUnited Sates\nBrazil',
    mapping: 'United States\nCanada',
    column: 'country',
    delimiter: 'auto',
    header: 'true',
    ignore_case: 'true',
    ignore_spacing: 'true',
    unmatched: 'fuzzy',
    fuzzy_threshold: '80',
    output: 'csv',
  });
  await page.goto(`/tools/category-canonicalize/?${params.toString()}`);
  await expect(page.locator('#in-unmatched')).toHaveValue('fuzzy', { timeout: 15_000 });
  await expect(page.locator('#in-header')).toBeChecked();
  await expect(page.locator('#in-fuzzy_threshold')).toHaveValue('80');
  await expect(page.locator('#tool-output')).toHaveText('country\nCanada\nUnited States\nBrazil', { timeout: 15_000 });
});

test('category-canonicalize suggestions output lists uncovered values for review', async ({ page }) => {
  await page.goto('/tools/category-canonicalize/');
  await page.fill('#in-data', 'country\nUSA\nCanadaa\nCanadaa\nUnited Sates');
  await page.fill('#in-mapping', 'United States\nCanada');
  await page.fill('#in-column', 'country');
  await page.check('#in-header');
  await page.selectOption('#in-output', 'suggestions');
  await expect(page.locator('#tool-output')).toHaveText('value,count,suggestion,similarity\nCanadaa,2,Canada,85.7\nUnited Sates,1,United States,92.3\nUSA,1,United States,23.1', { timeout: 15_000 });
});
