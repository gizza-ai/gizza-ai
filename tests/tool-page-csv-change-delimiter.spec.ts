import { test, expect } from './fixtures';
test('csv-change-delimiter page comma->semicolon', async ({ page }) => {
  await page.goto('/tools/csv-change-delimiter/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.fill('#in-to', ';');
  await expect(page.locator('#tool-output')).toHaveText('a;b\n1;2', { timeout: 15000 });
});
test('csv-change-delimiter query-param deep-link (to=tab)', async ({ page }) => {
  await page.goto('/tools/csv-change-delimiter/?data=' + encodeURIComponent('a,b\n1,2') + '&to=tab');
  await expect(page.locator('#in-data')).toHaveValue('a,b\n1,2', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('a\tb\n1\t2', { timeout: 15000 });
});
