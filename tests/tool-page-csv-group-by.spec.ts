import { test, expect } from './fixtures';
test('csv-group-by page sums per group', async ({ page }) => {
  await page.goto('/tools/csv-group-by/');
  await page.fill('#in-data', 'dept,amount\nA,10\nB,5\nA,20');
  await page.fill('#in-group_by_cols', 'dept');
  await page.fill('#in-aggregations', 'amount:sum, count');
  await expect(page.locator('#tool-output')).toHaveText('dept,sum_amount,count\nA,30,2\nB,5,1', { timeout: 15000 });
});
test('csv-group-by query-param deep-link', async ({ page }) => {
  await page.goto('/tools/csv-group-by/?data=' + encodeURIComponent('g,v\nx,2\nx,4') + '&group_by_cols=g&aggregations=' + encodeURIComponent('v:avg'));
  await expect(page.locator('#in-group_by_cols')).toHaveValue('g', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('g,avg_v\nx,3', { timeout: 15000 });
});
