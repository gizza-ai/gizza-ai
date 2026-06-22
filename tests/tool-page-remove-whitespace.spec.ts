import { test, expect } from './fixtures';

// /tools/remove-whitespace/ trims, collapses, or strips whitespace in-browser (pure wasm).
// text is a multiline <textarea>; mode + collapse_blank_lines are plain fields.

test('remove-whitespace page trims line edges (default mode)', async ({ page }) => {
  await page.goto('/tools/remove-whitespace/');
  await page.fill('#in-text', '  hello   world  \n   indented  ');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('hello   world\nindented', { timeout: 15000 });
});

test('remove-whitespace page collapses internal runs', async ({ page }) => {
  await page.goto('/tools/remove-whitespace/');
  await page.fill('#in-text', 'foo   bar\t\tbaz');
  await page.fill('#in-mode', 'collapse');
  await expect(page.locator('#tool-output')).toHaveText('foo bar baz', { timeout: 15000 });
});

test('remove-whitespace page strips all whitespace', async ({ page }) => {
  await page.goto('/tools/remove-whitespace/');
  await page.fill('#in-text', 'a b\tc\nd');
  await page.fill('#in-mode', 'strip');
  await expect(page.locator('#tool-output')).toHaveText('abcd', { timeout: 15000 });
});
