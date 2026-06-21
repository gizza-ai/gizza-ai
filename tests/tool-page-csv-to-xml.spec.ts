import { test, expect } from './fixtures';

// /tools/csv-to-xml/ converts CSV to XML in-browser (pure wasm).
test('csv-to-xml produces XML records with header tags', async ({ page }) => {
  await page.goto('/tools/csv-to-xml/');
  await page.fill('#in-data', 'name,age\nAda,36');
  await page.fill('#in-root', 'people');
  await page.fill('#in-row', 'person');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<people>', { timeout: 15000 });
  await expect(out).toContainText('<person>');
  await expect(out).toContainText('<name>Ada</name>');
  await expect(out).toContainText('<age>36</age>');
});
