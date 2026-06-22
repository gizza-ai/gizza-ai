import { test, expect } from './fixtures';

// /tools/syntax-highlighter/ highlights code to styled HTML or ANSI (pure wasm).
test('syntax-highlighter renders inline-styled HTML', async ({ page }) => {
  await page.goto('/tools/syntax-highlighter/');
  await page.fill('#in-code', 'fn main() { let x = 1 < 2; }');
  await page.fill('#in-language', 'rust');
  await page.selectOption('#in-format', 'html');
  const out = page.locator('#tool-output');
  // The output is the HTML markup string (format=text page), so the page shows
  // the literal <pre>...</pre> with inline-styled <span>s.
  await expect(out).toContainText('<pre style=', { timeout: 15000 });
  await expect(out).toContainText('<span style=');
  // `<` in the source is entity-escaped in the produced markup.
  await expect(out).toContainText('&lt;');
});

test('syntax-highlighter emits ANSI escape codes', async ({ page }) => {
  await page.goto('/tools/syntax-highlighter/');
  await page.fill('#in-code', 'print("hi")');
  await page.fill('#in-language', 'python');
  await page.selectOption('#in-format', 'ansi');
  const out = page.locator('#tool-output');
  // 24-bit ("\x1b[38;2;...m") true-color escapes appear in the output text.
  await expect(out).toContainText('[38;2;', { timeout: 15000 });
  await expect(out).toContainText('print');
});
