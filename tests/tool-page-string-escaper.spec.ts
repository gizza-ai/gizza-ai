import { test, expect } from './fixtures';

// /tools/string-escaper/ escapes/unescapes for a chosen target in-browser (pure wasm).

test('string-escaper escapes for JSON', async ({ page }) => {
  await page.goto('/tools/string-escaper/');
  await page.fill('#in-text', 'He said "hi"\nbye');
  await page.selectOption('#in-target', 'json');
  await page.selectOption('#in-mode', 'escape');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('He said \\"hi\\"\\nbye', { timeout: 15000 });
});

test('string-escaper escapes for HTML', async ({ page }) => {
  await page.goto('/tools/string-escaper/');
  await page.fill('#in-text', '<a href="x">&');
  await page.selectOption('#in-target', 'html');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('&lt;a href=&quot;x&quot;&gt;&amp;', { timeout: 15000 });
});

test('string-escaper wraps SQL literal in quotes', async ({ page }) => {
  await page.goto('/tools/string-escaper/');
  await page.fill('#in-text', "O'Brien");
  await page.selectOption('#in-target', 'sql');
  await page.check('#in-quotes');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText("'O''Brien'", { timeout: 15000 });
});

test('string-escaper unescapes a URL component', async ({ page }) => {
  await page.goto('/tools/string-escaper/');
  await page.fill('#in-text', 'S%C3%A3o+Paulo');
  await page.selectOption('#in-target', 'url');
  await page.selectOption('#in-mode', 'unescape');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('São Paulo', { timeout: 15000 });
});
