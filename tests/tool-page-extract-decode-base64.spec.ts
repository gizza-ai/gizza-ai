import { test, expect } from './fixtures';

// /tools/extract-decode-base64/ finds + decodes base64 in-browser (pure wasm).
test('extract-decode-base64 decodes a text token and a binary blob', async ({ page }) => {
  await page.goto('/tools/extract-decode-base64/');
  await page.fill(
    '#in-text',
    'auth: Basic SGVsbG8sIHdvcmxkISBUaGlzIGlzIGEgdGVzdC4= and png iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB here',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 Base64 blob(s) found', { timeout: 15000 });
  await expect(out).toContainText('Hello, world! This is a test.');
  await expect(out).toContainText('image/png');
  await expect(out).toContainText('89504e47');
});

test('extract-decode-base64 reports when none present', async ({ page }) => {
  await page.goto('/tools/extract-decode-base64/');
  await page.fill('#in-text', 'just some short words here');
  await expect(page.locator('#tool-output')).toContainText('No decodable', { timeout: 15000 });
});
