import { test, expect } from './fixtures';

// /tools/regex-extract/ returns every regex match in-browser (pure wasm).
test('regex-extract lists all matches', async ({ page }) => {
  await page.goto('/tools/regex-extract/');
  await page.fill('#in-text', 'a1 b2 c3');
  await page.fill('#in-pattern', '\\d');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 match(es)', { timeout: 15000 });
  await expect(out).toContainText('1');
  await expect(out).toContainText('2');
  await expect(out).toContainText('3');
});

test('regex-extract extracts a capture group', async ({ page }) => {
  await page.goto('/tools/regex-extract/');
  await page.fill('#in-text', 'key=val foo=bar');
  await page.fill('#in-pattern', '(\\w+)=(\\w+)');
  await page.fill('#in-capture_group', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 match(es)', { timeout: 15000 });
  await expect(out).toContainText('val');
  await expect(out).toContainText('bar');
});

test('regex-extract dedupes with unique', async ({ page }) => {
  await page.goto('/tools/regex-extract/');
  await page.fill('#in-text', 'a a b a c');
  await page.fill('#in-pattern', '\\w');
  await page.check('#in-unique');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 unique match(es)', { timeout: 15000 });
});
