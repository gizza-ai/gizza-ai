import { test, expect } from './fixtures';

// /tools/remove-duplicate-lines/ removes duplicate lines in-browser (pure wasm).
test('remove-duplicate-lines keeps first occurrence, preserves order', async ({ page }) => {
  await page.goto('/tools/remove-duplicate-lines/');
  await page.fill('#in-text', 'apple\nbanana\napple\ncherry\nbanana');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('apple\nbanana\ncherry', { timeout: 15000 });
});

test('remove-duplicate-lines respects ignore-case + keep last', async ({ page }) => {
  await page.goto('/tools/remove-duplicate-lines/');
  await page.fill('#in-text', 'Foo\nbar\nfoo\nFoo');
  await page.check('#in-ignore_case');
  await page.selectOption('#in-keep', 'last');
  const out = page.locator('#tool-output');
  // last occurrence kept; case-folded so all "foo" variants collapse to the last "Foo"
  await expect(out).toHaveText('bar\nFoo', { timeout: 15000 });
});

test('remove-duplicate-lines consecutive-only (uniq) + remove blank lines', async ({ page }) => {
  await page.goto('/tools/remove-duplicate-lines/');
  await page.fill('#in-text', 'x\nx\n\ny\nx');
  await page.check('#in-adjacent_only');
  await page.check('#in-remove_empty');
  const out = page.locator('#tool-output');
  // blanks dropped; only consecutive x's collapse, the trailing x is kept
  await expect(out).toHaveText('x\ny\nx', { timeout: 15000 });
});
