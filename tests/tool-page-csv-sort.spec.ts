import { test, expect } from './fixtures';

test('csv-sort page sorts by column numerically', async ({ page }) => {
  await page.goto('/tools/csv-sort/');
  await page.fill('#in-data', 'name,age\nBob,30\nAda,36\nCy,4');
  await page.fill('#in-columns', 'age');
  await expect(page.locator('#tool-output')).toHaveText('name,age\nCy,4\nBob,30\nAda,36', { timeout: 15000 });
});

test('csv-sort multi-column with per-column direction', async ({ page }) => {
  await page.goto('/tools/csv-sort/');
  await page.fill('#in-data', 'dept,salary\neng,100\nhr,50\neng,200\nhr,90');
  await page.fill('#in-columns', 'dept:asc,salary:desc');
  await expect(page.locator('#tool-output')).toHaveText('dept,salary\neng,200\neng,100\nhr,90\nhr,50', { timeout: 15000 });
});

test('csv-sort query-param deep-link (descending)', async ({ page }) => {
  await page.goto('/tools/csv-sort/?data=' + encodeURIComponent('n\n1\n3\n2') + '&columns=n&order=desc&numeric=number');
  await expect(page.locator('#in-data')).toHaveValue('n\n1\n3\n2', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('n\n3\n2\n1', { timeout: 15000 });
});
