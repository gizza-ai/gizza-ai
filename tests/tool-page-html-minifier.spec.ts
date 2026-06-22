import { test, expect } from './fixtures';

// /tools/html-minifier/ minifies HTML in-browser (pure wasm).
test('html-minifier collapses whitespace and strips comments', async ({ page }) => {
  await page.goto('/tools/html-minifier/');
  // "Remove comments" checkbox is checked by default (matches the chat default).
  await page.fill('#in-html', '<div>\n  <p>Hello <b>world</b></p>\n  <!-- note -->\n</div>');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('<div><p>Hello <b>world</b></p></div>', { timeout: 15000 });
});

test('html-minifier keeps comments when the box is unchecked', async ({ page }) => {
  await page.goto('/tools/html-minifier/');
  await page.uncheck('#in-remove_comments');
  await page.fill('#in-html', '<p>a</p><!-- keep -->');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<!-- keep -->', { timeout: 15000 });
});
