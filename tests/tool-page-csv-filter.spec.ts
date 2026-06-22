import { test, expect } from './fixtures';
test('csv-filter page numeric condition', async ({ page }) => {
  await page.goto('/tools/csv-filter/');
  await page.fill('#in-data', 'name,age\nAlice,30\nBob,25');
  await page.fill('#in-condition', 'age > 28');
  await expect(page.locator('#tool-output')).toHaveText('name,age\nAlice,30', { timeout: 15000 });
});
test('csv-filter query-param deep-link contains', async ({ page }) => {
  await page.goto('/tools/csv-filter/?data=' + encodeURIComponent('name\nAlice\nbob') + '&condition=' + encodeURIComponent('name contains al'));
  await expect(page.locator('#in-condition')).toHaveValue('name contains al', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('name\nAlice', { timeout: 15000 });
});
