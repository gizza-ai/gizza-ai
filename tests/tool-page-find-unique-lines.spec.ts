import { test, expect } from './fixtures';

// /tools/find-unique-lines/ keeps only lines appearing exactly once (pure wasm).
test('find-unique-lines keeps only one-off lines', async ({ page }) => {
  await page.goto('/tools/find-unique-lines/');
  await page.fill('#in-text', 'apple\nbanana\napple\ncherry\nbanana\ndate');
  const out = page.locator('#tool-output');
  // apple×2 and banana×2 drop out; cherry and date appear once, in order.
  await expect(out).toContainText('cherry', { timeout: 15000 });
  await expect(out).toContainText('date');
  await expect(out).not.toContainText('apple');
  await expect(out).not.toContainText('banana');
});

test('find-unique-lines respects ignore-case', async ({ page }) => {
  await page.goto('/tools/find-unique-lines/');
  // Foo/foo collapse to one repeated key → excluded; bar is the only one-off.
  await page.fill('#in-text', 'Foo\nfoo\nbar');
  await page.check('#in-ignore_case');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('bar', { timeout: 15000 });
  await expect(out).not.toContainText('Foo');
});
