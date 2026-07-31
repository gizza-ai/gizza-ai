import { test, expect } from './fixtures';

const SAMPLE = [
  'diff --git a/src/main.rs b/src/main.rs',
  '--- a/src/main.rs',
  '+++ b/src/main.rs',
  '@@ -1,3 +1,4 @@',
  ' fn main() {',
  '-    println!("hi");',
  '+    println!("hello");',
  '+    println!("world");',
  ' }',
].join('\n');

async function setDiff(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-diff').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('diff-viewer renders an exact inline view', async ({ page }) => {
  await page.goto('/tools/diff-viewer/');
  await setDiff(page, SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('1 file changed', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`1 file changed, 2 insertions(+), 1 deletion(-)

src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!("hi");
+    println!("hello");
+    println!("world");
 }`);
});

test('diff-viewer side-by-side view aligns replacements and additions', async ({ page }) => {
  await page.goto('/tools/diff-viewer/');
  await setDiff(page, SAMPLE);
  await page.selectOption('#in-view', 'side-by-side');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('println!("hello");', { timeout: 15_000 });
  const text = await out.textContent();
  expect(text).toContain('2 ~     println!("hi");');
  expect(text).toContain('2 ~     println!("hello");');
  expect(text).toContain('|     3 >     println!("world");');
});

test('diff-viewer deep-link applies stats view and ignore_whitespace', async ({ page }) => {
  const diff = [
    '--- a/app.py',
    '+++ b/app.py',
    '@@ -1,3 +1,3 @@',
    ' def main():',
    '-  return 1',
    '+    return 1',
    ' done',
  ].join('\n');
  const qs = new URLSearchParams({
    diff,
    view: 'stats',
    ignore_whitespace: 'true',
  });
  await page.goto(`/tools/diff-viewer/?${qs.toString()}`);

  await expect(page.locator('#in-diff')).toHaveValue(diff);
  await expect(page.locator('#in-view')).toHaveValue('stats');
  await expect(page.locator('#in-ignore_whitespace')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('1 file changed', { timeout: 15_000 });
  expect(await out.textContent()).toBe(' app.py |    0 \n 1 file changed');
});
