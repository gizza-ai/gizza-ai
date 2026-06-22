import { test, expect } from './fixtures';
test('csv-insert-column page appends a column', async ({ page }) => {
  await page.goto('/tools/csv-insert-column/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.fill('#in-name', 'c');
  await page.fill('#in-value', 'x');
  await expect(page.locator('#tool-output')).toHaveText('a,b,c\n1,2,x', { timeout: 15000 });
});
test('csv-insert-column query-param deep-link at front', async ({ page }) => {
  await page.goto('/tools/csv-insert-column/?data=' + encodeURIComponent('a,b\n1,2') + '&name=id&value=0&position=1');
  await expect(page.locator('#in-name')).toHaveValue('id', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('id,a,b\n0,1,2', { timeout: 15000 });
});
