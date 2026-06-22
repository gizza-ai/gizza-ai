import { test, expect } from './fixtures';

// /tools/csv-to-yaml/ converts CSV to a YAML list in-browser (pure wasm).
test('csv-to-yaml produces a YAML list of objects', async ({ page }) => {
  await page.goto('/tools/csv-to-yaml/');
  await page.fill('#in-data', 'name,age\nAda,36\nBo,40');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('name: Ada', { timeout: 15000 });
  await expect(out).toContainText('age: 36');
  await expect(out).toContainText('name: Bo');
});
