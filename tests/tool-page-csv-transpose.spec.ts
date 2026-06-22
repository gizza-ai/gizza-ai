import { test, expect } from './fixtures';

// /tools/csv-transpose/ swaps rows and columns in-browser (pure wasm).
test('csv-transpose swaps rows and columns', async ({ page }) => {
  await page.goto('/tools/csv-transpose/');
  await page.fill('#in-data', 'name,age\nAda,36\nBo,40');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('name,Ada,Bo', { timeout: 15000 });
  await expect(out).toContainText('age,36,40');
});
