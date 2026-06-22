import { test, expect } from './fixtures';

// /tools/list-converter/ reformats lists in-browser (pure wasm).
test('list-converter page converts comma list to numbered', async ({ page }) => {
  await page.goto('/tools/list-converter/');
  await page.fill('#in-input', 'banana, apple, cherry');
  await page.selectOption('#in-output_format', 'numbered');
  await expect(page.locator('#tool-output')).toContainText('1. banana', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('3. cherry');
});

test('list-converter page sort+dedupe via deep-link', async ({ page }) => {
  const qs = '?input=' + encodeURIComponent('b, a, b, c') + '&output_format=comma&sort=true&dedupe=true';
  await page.goto('/tools/list-converter/' + qs);
  await expect(page.locator('#tool-output')).toContainText('a, b, c', { timeout: 15000 });
});
