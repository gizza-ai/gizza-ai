import { test, expect } from './fixtures';

// /tools/js-minify/ minifies JavaScript in-browser (pure wasm).
test('js-minify strips whitespace and comments by default', async ({ page }) => {
  await page.goto('/tools/js-minify/');
  // "Remove comments" checkbox is checked by default (matches the chat default).
  await page.fill('#in-code', 'function f() {\n  // a comment\n  return a ? 1 : 2;\n}');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('function f(){return a?1:2;}', { timeout: 15000 });
});

test('js-minify keeps comments when the box is unchecked', async ({ page }) => {
  await page.goto('/tools/js-minify/');
  await page.uncheck('#in-remove_comments');
  await page.fill('#in-code', 'var a = 1; // note');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('// note', { timeout: 15000 });
});

test('js-minify preserves a /*! license banner even while stripping comments', async ({ page }) => {
  await page.goto('/tools/js-minify/');
  // remove_comments stays checked; keep_license is checked by default.
  await page.fill('#in-code', '/*! (c) 2026 ACME */\nfunction f() { return 1; }');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('/*! (c) 2026 ACME */function f(){return 1;}', { timeout: 15000 });
});
