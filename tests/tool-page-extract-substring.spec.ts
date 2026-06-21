import { test, expect } from './fixtures';

// /tools/extract-substring/ slices text in-browser (pure wasm).
test('extract-substring index mode with negative start', async ({ page }) => {
  await page.goto('/tools/extract-substring/');
  await page.fill('#in-text', 'hello world');
  await page.selectOption('#in-mode', 'index');
  await page.fill('#in-start', '-5');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('world', { timeout: 15000 });
});

test('extract-substring delimiters mode returns all matches', async ({ page }) => {
  await page.goto('/tools/extract-substring/');
  await page.fill('#in-text', 'a[1]b[2]c[3]');
  await page.selectOption('#in-mode', 'delimiters');
  await page.fill('#in-start_delim', '[');
  await page.fill('#in-end_delim', ']');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 match(es)', { timeout: 15000 });
  await expect(out).toContainText('[1] 1');
  await expect(out).toContainText('[3] 3');
});
