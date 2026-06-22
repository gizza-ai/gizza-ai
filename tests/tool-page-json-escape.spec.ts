import { test, expect } from './fixtures';

// /tools/json-escape/ escapes/unescapes for JSON in-browser (pure wasm).
test('json-escape escapes quotes and newlines', async ({ page }) => {
  await page.goto('/tools/json-escape/');
  await page.fill('#in-text', 'He said "hi"\nbye');
  await page.selectOption('#in-mode', 'escape');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('He said \\"hi\\"\\nbye', { timeout: 15000 });
});

test('json-escape unescapes back to raw text', async ({ page }) => {
  await page.goto('/tools/json-escape/');
  await page.fill('#in-text', 'a\\tb\\nc');
  await page.selectOption('#in-mode', 'unescape');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('a\tb\nc', { timeout: 15000 });
});
