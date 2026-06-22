import { test, expect } from './fixtures';

// /tools/sort-lines/ sorts lines in-browser (pure wasm).
test('sort-lines sorts alphabetically ascending', async ({ page }) => {
  await page.goto('/tools/sort-lines/');
  await page.fill('#in-text', 'banana\nApple\ncherry\napple');
  await page.selectOption('#in-method', 'alpha');
  await page.selectOption('#in-order', 'asc');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('Apple\napple\nbanana\ncherry', { timeout: 15000 });
});

test('sort-lines numeric descending', async ({ page }) => {
  await page.goto('/tools/sort-lines/');
  await page.fill('#in-text', '1 c\n10 a\n2 b');
  await page.selectOption('#in-method', 'numeric');
  await page.selectOption('#in-order', 'desc');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('10 a\n2 b\n1 c', { timeout: 15000 });
});

test('sort-lines natural order with remove duplicates', async ({ page }) => {
  await page.goto('/tools/sort-lines/');
  await page.fill('#in-text', 'file10\nfile2\nfile2\nfile1');
  await page.selectOption('#in-method', 'natural');
  await page.check('#in-unique');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('file1\nfile2\nfile10', { timeout: 15000 });
});
