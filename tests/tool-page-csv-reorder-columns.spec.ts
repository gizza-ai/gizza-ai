import { test, expect } from './fixtures';

// /tools/csv-reorder-columns/ reorders CSV columns in-browser (pure wasm).
// header is a checkbox (default on); the rest are fields.
test('csv-reorder-columns reorders and drops columns by name', async ({ page }) => {
  await page.goto('/tools/csv-reorder-columns/');
  await page.fill('#in-data', 'name,age,city\nAda,36,London');
  await page.fill('#in-columns', 'city,name');
  const out = page.locator('#tool-output');
  // age is dropped; order is city then name.
  await expect(out).toContainText('city,name', { timeout: 15000 });
  await expect(out).toContainText('London,Ada');
  await expect(out).not.toContainText('36');
});
