import { test, expect } from './fixtures';

// /tools/join-lines/ joins lines into one in-browser (pure wasm).
test('join-lines joins with comma-space', async ({ page }) => {
  await page.goto('/tools/join-lines/');
  await page.fill('#in-text', 'a\nb\nc');
  await page.fill('#in-separator', ', ');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('a, b, c', { timeout: 15000 });
});

test('join-lines trims and removes blank lines with a pipe', async ({ page }) => {
  await page.goto('/tools/join-lines/');
  await page.fill('#in-text', '  x  \n\n y ');
  await page.fill('#in-separator', '|');
  await page.check('#in-trim');
  await page.check('#in-remove_blank');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('x|y', { timeout: 15000 });
});

test('join-lines wraps each line with prefix/suffix (SQL IN clause)', async ({ page }) => {
  await page.goto('/tools/join-lines/');
  await page.fill('#in-text', '1\n2\n3');
  await page.fill('#in-separator', ', ');
  await page.fill('#in-prefix', "'");
  await page.fill('#in-suffix', "'");
  const out = page.locator('#tool-output');
  await expect(out).toHaveText("'1', '2', '3'", { timeout: 15000 });
});
