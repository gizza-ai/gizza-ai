import { test, expect } from './fixtures';

// /tools/find-duplicate-lines/ counts repeated lines in-browser (pure wasm).
test('find-duplicate-lines lists repeats with counts', async ({ page }) => {
  await page.goto('/tools/find-duplicate-lines/');
  await page.fill('#in-text', 'apple\nbanana\napple\ncherry\nbanana\napple');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 duplicated line(s)', { timeout: 15000 });
  await expect(out).toContainText('3 × apple');
  await expect(out).toContainText('2 × banana');
});

test('find-duplicate-lines respects ignore-case', async ({ page }) => {
  await page.goto('/tools/find-duplicate-lines/');
  await page.fill('#in-text', 'Foo\nfoo\nbar');
  await page.check('#in-ignore_case');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 × Foo', { timeout: 15000 });
});
