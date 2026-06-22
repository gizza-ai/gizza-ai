import { test, expect } from './fixtures';
test('csv-dedupe page removes full-row dupes', async ({ page }) => {
  await page.goto('/tools/csv-dedupe/');
  await page.fill('#in-data', 'name,age\nA,1\nA,1\nB,2');
  await expect(page.locator('#tool-output')).toHaveText('name,age\nA,1\nB,2', { timeout: 15000 });
});
test('csv-dedupe query-param deep-link keyed on column', async ({ page }) => {
  await page.goto('/tools/csv-dedupe/?data=' + encodeURIComponent('name,age\nA,1\nA,2\nB,3') + '&columns=name');
  await expect(page.locator('#in-data')).toHaveValue('name,age\nA,1\nA,2\nB,3', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('name,age\nA,1\nB,3', { timeout: 15000 });
});
