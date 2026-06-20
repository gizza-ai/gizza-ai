import { test, expect } from './fixtures';
test('csv-formula-eval page adds a computed column', async ({ page }) => {
  await page.goto('/tools/csv-formula-eval/');
  await page.fill('#in-data', 'price,qty\n10,3');
  await page.fill('#in-formulas', 'total = price * qty');
  await expect(page.locator('#tool-output')).toHaveText('price,qty,total\n10,3,30', { timeout: 15000 });
});
test('csv-formula-eval query-param deep-link', async ({ page }) => {
  await page.goto('/tools/csv-formula-eval/?data=' + encodeURIComponent('a,b\n2,3') + '&formulas=' + encodeURIComponent('s = a + b'));
  await expect(page.locator('#in-formulas')).toHaveValue('s = a + b', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('a,b,s\n2,3,5', { timeout: 15000 });
});
