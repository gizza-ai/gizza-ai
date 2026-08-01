import { test, expect } from './fixtures';

const SOURCE_A = `fn add(a: i32, b: i32) -> i32 {
    a + b
}`;

const SOURCE_B = `fn add(left: i32, right: i32) -> i32 {
    left + right
}`;

async function setField(
  page: import('@playwright/test').Page,
  id: string,
  value: string,
) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('ast-diff shows a real structural Rust change', async ({ page }) => {
  await page.goto('/tools/ast-diff/');
  await setField(page, '#in-a', SOURCE_A);
  await setField(page, '#in-b', SOURCE_B);
  await page.locator('#in-context').fill('1');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('--- a', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '--- a\n+++ b\n@@ -1,3 +1,3 @@\n-fn add(a: i32, b: i32) -> i32 {\n-    a + b\n+fn add(left: i32, right: i32) -> i32 {\n+    left + right\n }',
  );
});

test('ast-diff summary treats formatting and comments as non-structural', async ({ page }) => {
  await page.goto('/tools/ast-diff/');
  await setField(page, '#in-a', 'fn add(a:i32,b:i32)->i32{a+b}');
  await setField(
    page,
    '#in-b',
    'fn add(a: i32, b: i32) -> i32 {\n    // layout-only change\n    a + b\n}',
  );
  await page.selectOption('#in-mode', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('formatting-only', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'formatting-only: same program, differs only in formatting/whitespace/comments',
  );
});

test('ast-diff deep-link pre-fills summary mode and auto-runs', async ({ page }) => {
  const qs = new URLSearchParams({
    a: 'fn answer() -> i32 { 41 }',
    b: 'fn answer() -> i32 { 42 }',
    mode: 'summary',
    context: '0',
  });
  await page.goto(`/tools/ast-diff/?${qs.toString()}`);

  await expect(page.locator('#in-a')).toHaveValue('fn answer() -> i32 { 41 }');
  await expect(page.locator('#in-b')).toHaveValue('fn answer() -> i32 { 42 }');
  await expect(page.locator('#in-mode')).toHaveValue('summary');
  await expect(page.locator('#in-context')).toHaveValue('0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('changed:', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'changed: 1 line added, 1 line removed (canonical form)',
  );
});
