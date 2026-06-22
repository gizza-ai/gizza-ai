import { test, expect } from './fixtures';

const HTML =
  '<table><tr><th>Name</th><th>City</th></tr><tr><td>Alice</td><td>NYC</td></tr><tr><td>Bob</td><td>LA</td></tr></table>';

// /tools/html-table-extractor/ parses pasted HTML in-browser (pure wasm via
// scraper) and renders the chosen table as CSV (default) or JSON.
test('html-table-extractor page extracts a table to CSV', async ({ page }) => {
  await page.goto('/tools/html-table-extractor/');
  await page.fill('#in-html', HTML);
  await expect(page.locator('#tool-output')).toContainText('Alice,NYC', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Bob,LA');
});

test('html-table-extractor page emits JSON via deep-link', async ({ page }) => {
  const qs = '?html=' + encodeURIComponent(HTML) + '&format=json';
  await page.goto('/tools/html-table-extractor/' + qs);
  await expect(page.locator('#in-html')).toHaveValue(HTML, { timeout: 15000 });
  // JSON object form keyed by the header row.
  await expect(page.locator('#tool-output')).toContainText('"Name": "Alice"');
});
