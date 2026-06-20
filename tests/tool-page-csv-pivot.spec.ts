import { test, expect } from './fixtures';
test('csv-pivot page sum pivot', async ({ page }) => {
  await page.goto('/tools/csv-pivot/');
  await page.fill('#in-data', 'region,product,sales\nN,A,10\nN,B,5\nS,A,7');
  await page.fill('#in-rows', 'region');
  await page.fill('#in-columns', 'product');
  await page.fill('#in-values', 'sales');
  await expect(page.locator('#tool-output')).toHaveText('region,A,B\nN,10,5\nS,7,', { timeout: 15000 });
});
