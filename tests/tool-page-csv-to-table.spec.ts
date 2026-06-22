import { test, expect } from './fixtures';

// /tools/csv-to-table/ converts CSV to Markdown/HTML in-browser (pure wasm).
// format is a <select>; header a checkbox; data/delimiter fields.
test('csv-to-table produces a Markdown table', async ({ page }) => {
  await page.goto('/tools/csv-to-table/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.selectOption('#in-format', 'markdown');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('| a | b |', { timeout: 15000 });
  await expect(out).toContainText('| --- | --- |');
  await expect(out).toContainText('| 1 | 2 |');
});

test('csv-to-table produces an HTML table', async ({ page }) => {
  await page.goto('/tools/csv-to-table/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.selectOption('#in-format', 'html');
  await expect(page.locator('#tool-output')).toContainText('<th>a</th><th>b</th>', {
    timeout: 15000,
  });
});
