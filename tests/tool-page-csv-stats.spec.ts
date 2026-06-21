import { test, expect } from './fixtures';

// /tools/csv-stats/ computes per-column stats in-browser (pure wasm).
test('csv-stats reports numeric and text column stats', async ({ page }) => {
  await page.goto('/tools/csv-stats/');
  await page.fill('#in-data', 'name,age\nAda,36\nBo,40\nCy,36');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 data row(s)', { timeout: 15000 });
  await expect(out).toContainText('age: numeric');
  await expect(out).toContainText('min 36');
  await expect(out).toContainText('max 40');
  await expect(out).toContainText('name: text');
});
